export const SKIP_WAITING = "SKIP_WAITING" as const;
export const APP_READY = "APP_READY" as const;
export const UPDATE_TAKEOVER = "UPDATE_TAKEOVER" as const;
export const PREPARE_RELOAD = "PREPARE_RELOAD" as const;

export const SW_BROADCAST_CHANNEL = "easyquran-sw";
export const UPDATE_BROADCAST_CHANNEL = "easyquran-update";
export const PREPARE_RELOAD_EVENT = "easyquran-prepare-reload";

export type ClientToSwMessage = { type: typeof SKIP_WAITING } | { type: typeof APP_READY };

export type SwToClientMessage = { type: typeof UPDATE_TAKEOVER; version: string };

export type ClientToClientMessage = { type: typeof PREPARE_RELOAD };

export type UpdateMessage = SwToClientMessage | ClientToClientMessage;
