//! Passkeys on iOS and macOS over `ASAuthorizationController`. One request
//! runs at a time: a new one cancels the one showing, whose answer then
//! goes to the caller still waiting for it.

use std::cell::{Cell, RefCell};

use eq_core::passkey::{
    Assertion, Attestation, CreationOptions, Descriptor, ES256, RequestOptions, UserVerification,
};
use futures_channel::oneshot;
use nsh_core::prelude::run_on_main_thread;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject},
    sel,
};
use objc2_authentication_services::{
    ASAuthorization, ASAuthorizationController, ASAuthorizationControllerDelegate,
    ASAuthorizationControllerPresentationContextProviding, ASAuthorizationError,
    ASAuthorizationErrorDomain, ASAuthorizationPlatformPublicKeyCredentialAssertion,
    ASAuthorizationPlatformPublicKeyCredentialDescriptor,
    ASAuthorizationPlatformPublicKeyCredentialProvider,
    ASAuthorizationPlatformPublicKeyCredentialRegistration,
    ASAuthorizationPublicKeyCredentialAssertion,
    ASAuthorizationPublicKeyCredentialAssertionRequest,
    ASAuthorizationPublicKeyCredentialRegistration,
    ASAuthorizationPublicKeyCredentialRegistrationRequest,
    ASAuthorizationPublicKeyCredentialUserVerificationPreference,
    ASAuthorizationPublicKeyCredentialUserVerificationPreferenceDiscouraged,
    ASAuthorizationPublicKeyCredentialUserVerificationPreferencePreferred,
    ASAuthorizationPublicKeyCredentialUserVerificationPreferenceRequired, ASAuthorizationRequest,
    ASPublicKeyCredential,
};
use objc2_foundation::{NSArray, NSData, NSError, NSString};

use super::PasskeyError;

/// What the system's sheet shows over (`ASPresentationAnchor`).
#[cfg(target_os = "macos")]
type Anchor = nsh_platform_macos::objc2_app_kit::NSWindow;
#[cfg(target_os = "ios")]
type Anchor = objc2_ui_kit::UIWindow;

/// iOS 16: the first with synced passkeys. Every iOS build carries the
/// entitlement (`nsh.json`).
#[cfg(target_os = "ios")]
pub fn available() -> bool {
    use objc2_foundation::{NSOperatingSystemVersion, NSProcessInfo};
    NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
        majorVersion: 16,
        minorVersion: 0,
        patchVersion: 0,
    })
}

/// NSH's oldest macOS (14) has passkeys; the app may use them when it is
/// signed with a `webcredentials:` associated domain. Without it every
/// request fails, so they are not offered.
#[cfg(target_os = "macos")]
pub fn available() -> bool {
    use std::sync::OnceLock;
    static ENTITLED: OnceLock<bool> = OnceLock::new();
    *ENTITLED.get_or_init(entitlement::web_credentials)
}

#[cfg(target_os = "macos")]
mod entitlement {
    use std::{ffi::c_void, ptr};

    use objc2::{rc::Retained, runtime::AnyObject};
    use objc2_foundation::{NSArray, NSString};

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecTaskCreateFromSelf(allocator: *const c_void) -> *const c_void;
        fn SecTaskCopyValueForEntitlement(
            task: *const c_void,
            entitlement: *const NSString,
            error: *mut *const c_void,
        ) -> *mut AnyObject;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(object: *const c_void);
    }

    /// Whether this process is signed with a `webcredentials:` domain.
    pub fn web_credentials() -> bool {
        let key = NSString::from_str("com.apple.developer.associated-domains");
        // SAFETY: documented Security calls: a null allocator is the default
        // one, `NSString` is toll-free bridged to `CFString`, a null error is
        // allowed, and the copied value (or null) is owned by the caller.
        let value = unsafe {
            let task = SecTaskCreateFromSelf(ptr::null());
            if task.is_null() {
                return false;
            }
            let value = SecTaskCopyValueForEntitlement(task, &*key, ptr::null_mut());
            CFRelease(task);
            Retained::from_raw(value)
        };
        value
            .and_then(|value| value.downcast::<NSArray>().ok())
            .is_some_and(|domains| {
                domains.iter().any(|domain| {
                    domain
                        .downcast::<NSString>()
                        .is_ok_and(|text| text.to_string().starts_with("webcredentials:"))
                })
            })
    }
}

enum Ask {
    Register(CreationOptions),
    Assert(RequestOptions),
}

enum Answer {
    Made(Attestation),
    Signed(Assertion),
}

type Reply = Result<Answer, PasskeyError>;

pub async fn register(options: CreationOptions) -> Result<Attestation, PasskeyError> {
    // The platform authenticator makes ES256 keys only.
    if !options.algorithms.contains(&ES256) {
        return Err(PasskeyError::Unsupported);
    }
    match perform(Ask::Register(options)).await? {
        Answer::Made(attestation) => Ok(attestation),
        Answer::Signed(_) => Err(PasskeyError::Failed),
    }
}

pub async fn assert(options: RequestOptions) -> Result<Assertion, PasskeyError> {
    match perform(Ask::Assert(options)).await? {
        Answer::Signed(assertion) => Ok(assertion),
        Answer::Made(_) => Err(PasskeyError::Failed),
    }
}

async fn perform(ask: Ask) -> Reply {
    let (reply, answer) = oneshot::channel();
    run_on_main_thread(move || start(&ask, reply))
        .await
        .map_err(|_| PasskeyError::Failed)??;
    answer.await.map_err(|_| PasskeyError::Failed)?
}

/// The request showing and its delegate, which the controller holds weakly.
struct Running {
    controller: Retained<ASAuthorizationController>,
    _delegate: Retained<Delegate>,
}

thread_local! {
    /// The request showing (main thread).
    static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
}

/// Forgets `controller` once it answered; another one showing stays.
fn finished(controller: &ASAuthorizationController) -> Option<Running> {
    RUNNING.with_borrow_mut(|running| {
        running.take_if(|r| std::ptr::eq(&raw const *r.controller, controller))
    })
}

struct DelegateIvars {
    anchor: Retained<Anchor>,
    reply: Cell<Option<oneshot::Sender<Reply>>>,
}

define_class!(
    /// Hears the request's answer and names the window it shows over.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl ASAuthorizationControllerDelegate for Delegate {
        #[unsafe(method(authorizationController:didCompleteWithAuthorization:))]
        fn completed(
            &self,
            controller: &ASAuthorizationController,
            authorization: &ASAuthorization,
        ) {
            let _running = finished(controller);
            self.answer(answer_of(authorization));
        }

        #[unsafe(method(authorizationController:didCompleteWithError:))]
        fn failed(&self, controller: &ASAuthorizationController, error: &NSError) {
            let _running = finished(controller);
            self.answer(Err(error_of(error)));
        }
    }

    // Bound by the crate for macOS only, so the trait has no item for it;
    // it is still defined here, which registers it for the protocol.
    unsafe impl ASAuthorizationControllerPresentationContextProviding for Delegate {
        #[unsafe(method_id(presentationAnchorForAuthorizationController:))]
        fn anchor(&self, _controller: &ASAuthorizationController) -> Retained<Anchor> {
            self.ivars().anchor.clone()
        }
    }
);

impl Delegate {
    fn new(
        mtm: MainThreadMarker,
        anchor: Retained<Anchor>,
        reply: oneshot::Sender<Reply>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars {
            anchor,
            reply: Cell::new(Some(reply)),
        });
        // SAFETY: `NSObject`'s `init` on an object just allocated.
        unsafe { msg_send![super(this), init] }
    }

    fn answer(&self, reply: Reply) {
        if let Some(sender) = self.ivars().reply.take() {
            // The caller may have stopped waiting.
            let _ = sender.send(reply);
        }
    }
}

/// The error's code goes to the log; its text can name the app's site.
fn error_of(error: &NSError) -> PasskeyError {
    // SAFETY: a constant the framework defines.
    let domain = unsafe { ASAuthorizationErrorDomain };
    let code = error.code();
    if &*error.domain() != domain {
        log::warn!("passkeys: request failed with foreign error code {code}");
        return PasskeyError::Failed;
    }
    match ASAuthorizationError(code) {
        ASAuthorizationError::Canceled => PasskeyError::Cancelled,
        ASAuthorizationError::DeviceNotConfiguredForPasskeyCreation => PasskeyError::Unsupported,
        _ => {
            log::warn!("passkeys: request failed with ASAuthorizationError {code}");
            PasskeyError::Failed
        }
    }
}

fn answer_of(authorization: &ASAuthorization) -> Reply {
    // SAFETY: an authorization the framework completed with, read on the
    // main thread; each class answers the getters of the protocols it
    // conforms to.
    unsafe {
        let credential = authorization.credential();
        let object: &AnyObject = (*credential).as_ref();
        if let Some(made) =
            object.downcast_ref::<ASAuthorizationPlatformPublicKeyCredentialRegistration>()
        {
            let attestation = made.rawAttestationObject().ok_or(PasskeyError::Failed)?;
            return Ok(Answer::Made(Attestation {
                credential_id: made.credentialID().to_vec().into(),
                client_data_json: made.rawClientDataJSON().to_vec().into(),
                attestation_object: attestation.to_vec().into(),
                // The platform reports none.
                transports: Vec::new(),
            }));
        }
        if let Some(signed) =
            object.downcast_ref::<ASAuthorizationPlatformPublicKeyCredentialAssertion>()
        {
            let handle = signed.userID().to_vec();
            return Ok(Answer::Signed(Assertion {
                credential_id: signed.credentialID().to_vec().into(),
                client_data_json: signed.rawClientDataJSON().to_vec().into(),
                authenticator_data: signed.rawAuthenticatorData().to_vec().into(),
                signature: signed.signature().to_vec().into(),
                user_handle: (!handle.is_empty()).then(|| handle.into()),
            }));
        }
    }
    log::warn!("passkeys: the system answered with another kind of credential");
    Err(PasskeyError::Failed)
}

#[cfg(target_os = "ios")]
fn anchor(mtm: MainThreadMarker) -> Option<Retained<Anchor>> {
    nsh_platform_ios::presenting_controller(mtm)?
        .view()?
        .window()
}

#[cfg(target_os = "macos")]
fn anchor(mtm: MainThreadMarker) -> Option<Retained<Anchor>> {
    let app = nsh_platform_macos::objc2_app_kit::NSApplication::sharedApplication(mtm);
    app.keyWindow().or_else(|| app.mainWindow())
}

fn verification(
    wanted: UserVerification,
) -> Option<&'static ASAuthorizationPublicKeyCredentialUserVerificationPreference> {
    // SAFETY: constants the framework defines.
    unsafe {
        match wanted {
            UserVerification::Required => {
                ASAuthorizationPublicKeyCredentialUserVerificationPreferenceRequired
            }
            UserVerification::Preferred => {
                ASAuthorizationPublicKeyCredentialUserVerificationPreferencePreferred
            }
            UserVerification::Discouraged => {
                ASAuthorizationPublicKeyCredentialUserVerificationPreferenceDiscouraged
            }
        }
    }
}

fn descriptors(
    list: &[Descriptor],
) -> Retained<NSArray<ASAuthorizationPlatformPublicKeyCredentialDescriptor>> {
    let items: Vec<_> = list
        .iter()
        .map(|descriptor| {
            let id = NSData::with_bytes(descriptor.id.as_slice());
            // SAFETY: the class's initializer, with a credential id.
            unsafe {
                ASAuthorizationPlatformPublicKeyCredentialDescriptor::initWithCredentialID(
                    ASAuthorizationPlatformPublicKeyCredentialDescriptor::alloc(),
                    &id,
                )
            }
        })
        .collect();
    NSArray::from_retained_slice(&items)
}

fn provider(rp_id: &str) -> Retained<ASAuthorizationPlatformPublicKeyCredentialProvider> {
    // SAFETY: the class's initializer, with a relying party id.
    unsafe {
        ASAuthorizationPlatformPublicKeyCredentialProvider::initWithRelyingPartyIdentifier(
            ASAuthorizationPlatformPublicKeyCredentialProvider::alloc(),
            &NSString::from_str(rp_id),
        )
    }
}

fn request_of(ask: &Ask) -> Retained<ASAuthorizationRequest> {
    match ask {
        Ask::Register(options) => registration(options),
        Ask::Assert(options) => assertion(options),
    }
}

fn registration(options: &CreationOptions) -> Retained<ASAuthorizationRequest> {
    // SAFETY: documented factory method and setters of a request not yet
    // performed, on the main thread.
    unsafe {
        let request = provider(&options.rp_id)
            .createCredentialRegistrationRequestWithChallenge_name_userID(
                &NSData::with_bytes(options.challenge.as_slice()),
                &NSString::from_str(&options.user_name),
                &NSData::with_bytes(options.user_id.as_slice()),
            );
        if let Some(preference) = verification(options.user_verification) {
            request.setUserVerificationPreference(preference);
        }
        // iOS 17.4, macOS 14.4: before them a listed passkey is not refused.
        if !options.exclude.is_empty() && request.respondsToSelector(sel!(setExcludedCredentials:))
        {
            let excluded = descriptors(&options.exclude);
            let () = msg_send![&*request, setExcludedCredentials: &*excluded];
        }
        Retained::into_super(request)
    }
}

fn assertion(options: &RequestOptions) -> Retained<ASAuthorizationRequest> {
    // SAFETY: as in `registration`.
    unsafe {
        let request = provider(&options.rp_id).createCredentialAssertionRequestWithChallenge(
            &NSData::with_bytes(options.challenge.as_slice()),
        );
        if let Some(preference) = verification(options.user_verification) {
            request.setUserVerificationPreference(preference);
        }
        if !options.allow.is_empty() {
            request.setAllowedCredentials(&descriptors(&options.allow));
        }
        Retained::into_super(request)
    }
}

/// Shows the system's sheet for `ask`; `reply` hears its answer.
fn start(ask: &Ask, reply: oneshot::Sender<Reply>) -> Result<(), PasskeyError> {
    let mtm = MainThreadMarker::new().ok_or(PasskeyError::Failed)?;
    let Some(anchor) = anchor(mtm) else {
        log::warn!("passkeys: no window to show the system's sheet over");
        return Err(PasskeyError::Failed);
    };
    if let Some(earlier) = RUNNING.take() {
        // SAFETY: a controller this module performed, on the main thread.
        // Dropping it and its delegate ends the earlier caller's wait.
        unsafe { earlier.controller.cancel() };
    }
    let requests = NSArray::from_retained_slice(&[request_of(ask)]);
    let delegate = Delegate::new(mtm, anchor, reply);
    // SAFETY: the class's initializer and setters; the delegate is kept
    // alive with the controller (`Running`).
    let controller = unsafe {
        let controller = ASAuthorizationController::initWithAuthorizationRequests(
            ASAuthorizationController::alloc(),
            &requests,
        );
        controller.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        controller.setPresentationContextProvider(Some(ProtocolObject::from_ref(&*delegate)));
        controller
    };
    RUNNING.set(Some(Running {
        controller: controller.clone(),
        _delegate: delegate,
    }));
    // SAFETY: on the main thread, with its delegate and window set.
    unsafe { controller.performRequests() };
    Ok(())
}
