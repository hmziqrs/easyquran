# Device contract

What every client must do for account devices.

## Edge cases

| Case | Required behaviour |
| --- | --- |
| More than 20 devices | Oldest idle evicted, never the caller |
| Sign-out on a shared phone | Push token deleted first |
