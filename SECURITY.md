# Security Policy

RustyMeals is a personal project — an offline-first companion app for a
self-hosted Mealie instance, used by my household. It's not a commercial
product, but I still take security issues seriously, especially anything
touching how the Mealie API token is stored or transmitted.

## Supported Versions

Only the latest release is supported. Older releases won't receive
security fixes — please update to the newest version before reporting
an issue to confirm it's still present.

## Reporting a Vulnerability

Please **do not open a public GitHub issue** for security vulnerabilities.

Instead, use GitHub's private vulnerability reporting:

1. Go to the [Security tab](../../security) of this repository
2. Click **"Report a vulnerability"**

This opens a private report visible only to me, so details aren't
exposed before a fix is available.



## What I'd consider in scope

- Anything that could expose the Mealie API token or server URL to a
  third party
- Anything that could let one device read or modify another
  household member's data
- Standard issues: injection, path traversal, unsafe deserialization,
  etc.

## Response expectations

This is a side project I maintain in my spare time, so I can't promise
a fixed turnaround — but I'll acknowledge reports as soon as I see
them and aim to patch confirmed issues promptly, especially anything
involving credential exposure.
