//! The local fake API backing the data feature: a deterministic dataset of
//! posts, authors, likes and comments, with latency, failure and offline
//! switches, so no screen or test needs a server or randomness.

use std::time::Duration;

use nsh_core::prelude::*;

/// Posts in one page of the feed.
pub const PAGE_SIZE: u64 = 10;

/// How long a detail query's answer stays fresh, in milliseconds; after it,
/// a served cache entry revalidates.
pub const DETAIL_TTL_MS: u64 = 2_000;

const POST_COUNT: u64 = 30;

const TITLES: [&str; 10] = [
    "Shipping a Rust shell",
    "Why cells recycle",
    "Signals over snapshots",
    "The five-minute query",
    "Pull to refresh, properly",
    "Optimistic hearts",
    "A tracer's notebook",
    "Latency you can hold",
    "The offline banner",
    "Keys keep the state",
];

const BODIES: [&str; 10] = [
    "One turn of the app's code becomes one commit, and the commit is what the screen shows.",
    "A cell is handed another row, and only the bindings that read the position run again.",
    "A value read in a body is a photograph; a closure where a value goes stays current.",
    "Serve yesterday's answer while today's is on its way, and nobody watches a spinner.",
    "The spinner is the platform's; the state is one bool the app owns.",
    "Show the like at once, take it back if the write fails, and never block the tap.",
    "Mount items are the honest number: deterministic, small, and easy to defend.",
    "Two hundred milliseconds is enough to see every state a screen can be in.",
    "Gray the feed, say why, and fail the writes fast so the UI can say so too.",
    "Identity is a domain id, never a position and never a generated value.",
];

const AUTHORS: [&str; 6] = [
    "Ada Lovelace",
    "Grace Hopper",
    "Alan Kay",
    "Barbara Liskov",
    "Dennis Ritchie",
    "Margaret Hamilton",
];

const COMMENT_BODIES: [&str; 6] = [
    "This matches what we measured.",
    "The rollback path sold me.",
    "Filed a trace for our feed: flat nodes.",
    "Would read a whole post on this.",
    "We shipped the same pattern last week.",
    "The manual clock makes this testable.",
];

/// One failure mode of the fake API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// The offline switch is on.
    Offline,
    /// The failure switch picked this request.
    Failed,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offline => write!(f, "You are offline"),
            Self::Failed => write!(f, "The request failed"),
        }
    }
}

/// A post of the feed. `revision` counts reads of the post, so a refetch
/// after a cache serve is visible in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Post {
    pub id: u64,
    pub author: String,
    pub title: String,
    pub body: String,
    pub likes: u64,
    pub liked: bool,
    pub comments: u64,
    pub revision: u64,
}

/// A comment on a post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: u64,
    pub post_id: u64,
    pub author: String,
    pub body: String,
}

/// One page of the feed; `next_cursor` is the page index after this one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostPage {
    pub posts: Vec<Post>,
    pub next_cursor: Option<u64>,
}

/// The fake API's state: its dataset and its switches. Held in one global
/// signal, so the devtools page can turn the switches at run time.
#[derive(Debug, Clone)]
pub struct FakeApi {
    posts: Vec<Post>,
    comments: Vec<Comment>,
    /// Delay every response waits before answering.
    pub latency_ms: u64,
    /// Every Nth request fails; 0 never fails.
    pub fail_every: u32,
    pub offline: bool,
    /// Requests counted by the gate, failed ones included.
    pub requests: u64,
    /// Requests the failure switch rejected.
    pub failures: u64,
    /// Responses the test can advance through the latency.
    pub in_flight: u64,
    next_comment_id: u64,
}

impl Default for FakeApi {
    fn default() -> Self {
        let mut posts = Vec::new();
        let mut comments = Vec::new();
        let mut next_comment_id = 1;
        for i in 0..POST_COUNT {
            let id = i + 1;
            let count = i % 4;
            for j in 0..count {
                comments.push(Comment {
                    id: next_comment_id,
                    post_id: id,
                    author: AUTHORS[((i + j) % AUTHORS.len() as u64) as usize].to_owned(),
                    body: COMMENT_BODIES[((i + j) % COMMENT_BODIES.len() as u64) as usize]
                        .to_owned(),
                });
                next_comment_id += 1;
            }
            posts.push(Post {
                id,
                author: AUTHORS[(i % AUTHORS.len() as u64) as usize].to_owned(),
                title: TITLES[(i % TITLES.len() as u64) as usize].to_owned(),
                body: BODIES[(i % BODIES.len() as u64) as usize].to_owned(),
                likes: (i * 7 + 3) % 40,
                liked: i % 5 == 0,
                comments: count,
                // The first read reports 1, so `post()` bumps before returning.
                revision: 0,
            });
        }
        Self {
            posts,
            comments,
            latency_ms: 100,
            fail_every: 0,
            offline: false,
            requests: 0,
            failures: 0,
            in_flight: 0,
            next_comment_id,
        }
    }
}

/// The surface's fake API. Every call returns the same state.
pub fn fake_api() -> Signal<FakeApi> {
    global::<FakeApi>()
}

impl FakeApi {
    fn post(&mut self, id: u64) -> Option<&mut Post> {
        let post = self.posts.iter_mut().find(|post| post.id == id)?;
        // Reads bump `revision`, so a refetch is visible after a cache serve.
        post.revision += 1;
        Some(post)
    }

    fn page(&self, cursor: u64) -> PostPage {
        let start = (cursor * PAGE_SIZE) as usize;
        let posts: Vec<Post> = self
            .posts
            .iter()
            .skip(start)
            .take(PAGE_SIZE as usize)
            .cloned()
            .collect();
        let next_cursor = (start + posts.len() < self.posts.len()).then_some(cursor + 1);
        PostPage { posts, next_cursor }
    }

    fn comments_for(&self, post_id: u64) -> Vec<Comment> {
        self.comments
            .iter()
            .filter(|comment| comment.post_id == post_id)
            .cloned()
            .collect()
    }
}

/// Runs one request: counts it, decides failure deterministically, waits the
/// latency, then reads the answer from the dataset as it is at completion.
async fn respond<T>(work: impl FnOnce(&mut FakeApi) -> T) -> Result<T, ApiError> {
    // `Signal::update` returns nothing, so the gate's decision leaves through
    // a cell; everything here runs on the one engine thread.
    let decision = std::cell::Cell::new((false, false, 0_u64));
    fake_api().update(|api| {
        api.requests += 1;
        api.in_flight += 1;
        let fail =
            !api.offline && api.fail_every > 0 && api.requests % u64::from(api.fail_every) == 0;
        if fail {
            api.failures += 1;
        }
        decision.set((api.offline, fail, api.latency_ms));
    });
    let (offline, fail, latency_ms) = decision.get();
    let finish = |api: &mut FakeApi| api.in_flight = api.in_flight.saturating_sub(1);
    // A rejected request touches nothing: the dataset work only runs on a
    // delivered response.
    if offline {
        fake_api().update(finish);
        return Err(ApiError::Offline);
    }
    timer(Duration::from_millis(latency_ms)).await;
    if fail {
        fake_api().update(finish);
        return Err(ApiError::Failed);
    }
    let answer = std::cell::RefCell::new(None);
    fake_api().update(|api| {
        finish(api);
        answer.borrow_mut().replace(work(api));
    });
    Ok(answer
        .into_inner()
        .expect("the work runs whenever the state updates"))
}

/// The feed's page at `cursor` (0, 1, 2, ...).
pub async fn posts_page(cursor: u64) -> Result<PostPage, ApiError> {
    respond(|api| api.page(cursor)).await
}

/// One post; each successful read bumps its `revision`.
pub async fn post(id: u64) -> Result<Post, ApiError> {
    respond(|api| {
        api.post(id)
            .map(|post| post.clone())
            .expect("the dataset holds every id the pages hand out")
    })
    .await
}

/// The comments on a post, oldest first.
pub async fn comments(post_id: u64) -> Result<Vec<Comment>, ApiError> {
    respond(|api| api.comments_for(post_id)).await
}

/// Sets a post's liked state and adjusted count, and returns the post.
pub async fn like(post_id: u64, to: bool) -> Result<Post, ApiError> {
    respond(|api| {
        let post = api
            .post(post_id)
            .expect("the dataset holds every id the pages hand out");
        if post.liked != to {
            post.likes = if to {
                post.likes.saturating_add(1)
            } else {
                post.likes.saturating_sub(1)
            };
            post.liked = to;
        }
        post.clone()
    })
    .await
}

/// Appends a comment and returns it.
pub async fn add_comment(post_id: u64, body: String) -> Result<Comment, ApiError> {
    respond(|api| {
        let id = api.next_comment_id;
        api.next_comment_id += 1;
        let comment = Comment {
            id,
            post_id,
            author: "You".to_owned(),
            body,
        };
        api.comments.push(comment.clone());
        if let Some(post) = api.posts.iter_mut().find(|post| post.id == post_id) {
            post.comments += 1;
        }
        comment
    })
    .await
}

/// Sets the latency every response waits.
pub fn set_latency(ms: u64) {
    fake_api().update(|api| api.latency_ms = ms);
}

/// Sets how many requests fail: 0 never, 1 all, N every Nth.
pub fn set_fail_every(n: u32) {
    fake_api().update(|api| api.fail_every = n);
}

/// Sets the offline switch; offline requests fail at once.
pub fn set_offline(offline: bool) {
    fake_api().update(|api| api.offline = offline);
}
