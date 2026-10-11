import type { MessagePayload } from "firebase/messaging";

export const PUSH_TITLE = "EasyQuran";
export const PUSH_BODY = "Open EasyQuran to view notifications.";
export const PUSH_TAG = "easyquran-notifications";

export function publicPushPayload(payload: MessagePayload): MessagePayload {
  return {
    from: payload.from,
    collapseKey: PUSH_TAG,
    messageId: payload.messageId,
    notification: { title: PUSH_TITLE, body: PUSH_BODY },
  };
}
