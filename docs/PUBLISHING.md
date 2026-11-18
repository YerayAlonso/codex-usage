# Publishing checklist

These steps are deliberately manual. Preparing this repository does not create a
GitHub repository, make a commit, push code, or publish a release.

## First source publication

1. Review the source, MIT license, staged files, and ignore rules:

   ```sh
   git status
   git diff --cached --check
   git diff --cached --stat
   npm run verify
   ```

   Do not include local data, logs, auth files, signing keys, installers, `dist`,
   `node_modules`, or `src-tauri/target`. Review the author identity and email that
   Git will put in the public commit; use your GitHub private email if desired.

2. Create the initial commit after reviewing it:

   ```sh
   git commit -m "Prepare Codex Usage for public release"
   ```

3. Check the account used by GitHub CLI. If the remote repository does not already
   exist, create it with the visibility you intend. For a **public** repository:

   ```sh
   gh repo create YerayAlonso/codex-usage --public
   ```

4. `origin` should already point to the intended URL. Check it, then push:

   ```sh
   git remote -v
   git push -u origin main
   ```

5. On GitHub, enable private vulnerability reporting and review the CI results.
   Consider requiring CI and review before merging changes to `main`.
   Do not describe macOS/Linux desktop behavior as verified until tested there.

## Binary releases

- Keep the package, Cargo, Tauri, and lockfile versions aligned.
- Build from the exact committed source you intend to release, not a stale local
  executable. Run the manual **Build packages** workflow for each intended platform.
- Download the Actions artifacts, check `SHA256SUMS.txt`, and test the installers.
  Hashes check integrity; they are not a publisher signature.
- Actions artifact archives may not preserve executable permissions. On Linux,
  restore the AppImage executable bit with `chmod +x <file>.AppImage` when needed.
- Explain unsigned-package warnings and clearly label unverified platform builds.
- Create a GitHub release and attach the tested packages and checksums manually.
  Neither the CI nor packaging workflow publishes a release automatically.
