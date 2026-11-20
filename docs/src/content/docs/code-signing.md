---
title: Code signing policy
description: "How Reach's Windows releases are signed: free code signing provided by SignPath.io, certificate by SignPath Foundation. Who builds, reviews and approves each signed release."
---

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

Signing is being set up now. Until the first signed release is out, Windows still shows a SmartScreen warning for the installer; the [download page](/download/) says how to get past it.

## What is signed

The Windows build of Reach: the program itself (`reach.exe`) and its two installers, the `.msi` and the `-setup.exe`. Nothing else is signed with this certificate, and only software built from this project's own source code is.

Every signed file is built by [GitHub Actions](https://github.com/alexandrosnt/Reach/actions) on GitHub-hosted runners, from the source in [this repository](https://github.com/alexandrosnt/Reach), by the release workflow in the same repository. SignPath checks that origin before it signs, and every release is approved by hand before anything is signed.

## Team roles

- **Committers and reviewers:** [Alexander Nt. (@alexandrosnt)](https://github.com/alexandrosnt). Changes from anyone else arrive as pull requests and are reviewed before they are merged.
- **Approvers:** [Alexander Nt. (@alexandrosnt)](https://github.com/alexandrosnt), who approves each signing request.

Everyone in these roles uses multi-factor authentication for GitHub and SignPath.

## Privacy

Reach has no account, no analytics and no telemetry. On its own it only asks GitHub whether a new version is out; every other connection is one you asked for. The [privacy policy](/privacy/) lists every connection the app makes, what is sent and to whom.

## Reporting a problem

If you think a file signed for Reach is not what it should be, open a [security advisory](https://github.com/alexandrosnt/Reach/security/advisories/new) (see the [security policy](https://github.com/alexandrosnt/Reach/blob/main/SECURITY.md)), or write to SignPath at support@signpath.io.
