# Petal development workflow

- Use the installed Graphify skill for code changes. Prefer local AST extraction and keep generated graphs out of Git.
- After each validated change, build the release executable, install it into the existing `Petal` project folder on the Windows Desktop, and restart Petal quietly. This is an explicit user preference; do not wait for a separate installation request.
- Resolve the Desktop using the Windows known folder rather than assuming a path; it may be redirected to OneDrive. Keep the existing project folder and settings.
- Avoid interrupting an active conversion when replacing the running executable.
- Preserve the single-executable distribution. Do not require adjacent assets or scripts for normal use.
