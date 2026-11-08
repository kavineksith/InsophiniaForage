# InsophiniaForage

> ⚠️ **EXPERIMENTAL SOFTWARE — USE AT YOUR OWN RISK**
>
> This is an **experimental, unfinished project** built to explore modern desktop dev tooling for PHP. It has **not been audited, security-reviewed, or tested in production**. There are no guarantees — express or implied — about correctness, stability, data safety, or fitness for any particular purpose.
>
> **Before using:**
> - **Back up your data.** The app spawns nginx, PHP-FPM, and MariaDB as child processes. Bugs could cause data loss, especially in `~/.insophinia-forage/data/mysql/`.
> - **Don't use it for production sites.** It's a local dev tool only. The MariaDB config uses `skip-grant-tables` (matches XAMPP's behavior) — safe on localhost, fatal if exposed.
> - **Don't trust it for client work.** Verify everything it generates — configs, `wp-config.php`, DB schemas.
> - **Expect breaking changes.** The settings schema, config templates, and on-disk layout may change between versions without migration.
>
> By using this software you accept full responsibility for any consequences. See [`DISCLAIMER`](./DISCLAIMER) for the full legal text.
>
> ---

A cross-platform desktop development stack — nginx + PHP + MariaDB + phpMyAdmin + Composer + WordPress — managed from a clean, Mac-style GUI built with Tauri 2 (Rust) + SvelteKit.

Think **XAMPP, but modern**: small installer, low memory, no root required, and a UI that doesn't look like it was built in 2005.

> Primary target: **Linux Mint** (also runs on Ubuntu, Fedora, macOS, and Windows).

---

## Features

- **One-window service control** — start/stop nginx, PHP-FPM, MariaDB individually or all at once.
- **First-run wizard** — pick PHP/MariaDB/nginx versions, choose ports, choose binary strategy.
- **Hybrid binary strategy** — auto-detect system installs → fall back to bundled → fall back to download.
- **Project manager** — every folder in `~/.insophinia-forage/www/` is auto-served at `http://<name>:<port>`.
- **Per-project PHP version switcher** — each project can pin a different PHP version (e.g. 8.1 for legacy WP, 8.3 for new Laravel). The app spawns one PHP-FPM process per active version and routes via an nginx `map` directive.
- **Database manager** — create/drop databases, run SQL, or open phpMyAdmin in your browser.
- **One-click WordPress** — downloads WP, creates DB, writes `wp-config.php` with random salts.
- **Composer runner** — `composer install` from the GUI, scoped to a project.
- **Real-time log streaming** — uses the `notify` crate (inotify/FSEvents/ReadDirectoryChangesW) to push new log lines to the UI as they're written. No polling. Capped at 5,000 in-memory lines per service to prevent UI bloat.
- **System tray** — Start All / Stop All / Open Panel / Quit (services stop cleanly on quit).
- **Production logging** — `tracing` + daily rotation, 7-day retention, automatic sweeping.
- **No memory leaks** — child processes are tracked, killed on exit, and pump threads are joined. Log watchers are cleaned up on unsubscribe and on app quit.
- **No root needed** — uses unprivileged ports (1024+), per-user config, per-user data dir.
- **Bundle binaries into the installer** — run `npm run bundle:binaries` before `npm run tauri:build` to ship a fully offline-ready installer (~200 MB) with PHP, MariaDB, phpMyAdmin, and Composer pre-bundled.
- **Conflict resolution policy** — when a component (PHP, MariaDB, nginx) is already installed on your system, the app can **coexist** (install its own copy, leave system alone — default), **use system** (don't download, use what's there), or **replace** (uninstall the system package — not recommended). Set in Settings → General → Conflict policy.
- **Detection report** — before downloading anything, the app scans your system for existing installs and shows what it found (path + version), what's bundled, what's already downloaded, and what it plans to do. See Settings → Detection tab.
- **Version compatibility matrix** — the app enforces known constraints between components (e.g. phpMyAdmin 5.x needs PHP ≥ 7.2; WordPress 6.x needs PHP ≥ 7.2 recommended ≥ 7.4; MariaDB 11.x uses new auth plugin). The wizard refuses to save incompatible combinations; the Settings page shows warnings in real time.

---

## Architecture

```
┌────────────────────────────────────────────┐
│  SvelteKit UI (HTML/CSS/TS, ~50 KB)        │
│  ─────────────────────────────────────────  │
│  • Dashboard  • Projects  • Databases      │
│  • WordPress  • Logs     • Settings        │
│  • First-run wizard                        │
└────────────────┬───────────────────────────┘
                 │ invoke() / events
┌────────────────▼───────────────────────────┐
│  Rust backend (src-tauri/src/)             │
│  ─────────────────────────────────────────  │
│  commands/   Tauri command handlers        │
│  core/       AppState, paths, settings,    │
│              process mgr, config gen,      │
│              ports, logging                │
│  binaries/   detect + download + bundle    │
│  templates/  nginx.conf, php-fpm.conf,     │
│              php.ini, my.cnf               │
│  tray.rs     System tray                   │
└────────────────┬───────────────────────────┘
                 │ spawns + monitors
┌────────────────▼───────────────────────────┐
│  Child processes                           │
│  ─────────────────────────────────────────  │
│  nginx -c conf/nginx.conf                  │
│  php-fpm -F -y conf/php-fpm.conf           │
│  mariadbd --defaults-file=conf/my.cnf      │
└────────────────────────────────────────────┘
```

### Folder layout (per-user, no root)

```
~/.insophinia-forage/
├── bin/                 # downloaded/bundled binaries (php, nginx, mariadb, composer.phar)
├── conf/                # generated config files (regenerated on every settings change)
├── data/
│   ├── mysql/           # MariaDB data dir (chmod 700)
│   ├── sessions/        # PHP session save path
│   ├── uploads/         # PHP upload_tmp_dir
│   └── tmp/             # PHP sys_temp_dir
├── logs/
│   ├── app/             # tracing-appender logs, daily rotation, 7-day retention
│   ├── nginx/           # nginx access + error logs
│   ├── php/             # PHP-FPM error + slow logs
│   └── mariadb/         # MariaDB error log
├── www/                 # web roots (one folder per project)
├── cache/               # downloaded archives, extracted before install
├── run/                 # PID files, Unix sockets (php-fpm.sock, mariadb.sock)
└── settings.json        # user settings (chmod 600 — contains DB password)
```

---

## Development

### Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Node.js | ≥ 18 | for the SvelteKit frontend |
| Rust | ≥ 1.77 | `rustup default stable` |
| Tauri 2 system deps | see below | per-OS |

**Linux (Mint/Ubuntu/Debian):**
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
                 libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

**Fedora:**
```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
                 libappindicator-gtk3-devel librsvg2-devel
```

**macOS:** Xcode Command Line Tools (`xcode-select --install`).

**Windows:** Microsoft Visual C++ Build Tools + WebView2 runtime.

### Install & run

```bash
git clone <this-repo>
cd insophinia-forage
npm install
npm run tauri:dev
```

The first `npm run tauri:dev` will:
1. Build the SvelteKit frontend (Vite).
2. Compile the Rust binary (Cargo — slow on first run, ~3 min).
3. Open the desktop window.

### Type checking

```bash
npm run check
```

### Production build

```bash
npm run tauri:build
```

Outputs installers under `src-tauri/target/release/bundle/`:
- **Linux**: `.deb`, `.rpm`, `.AppImage`
- **macOS**: `.dmg`, `.app`
- **Windows**: `.msi`, `.exe` (NSIS)

### Building an offline-ready installer (with bundled binaries)

By default, the installer is small (~10 MB) and downloads PHP/MariaDB/etc. on first run. To ship a fully offline installer with binaries pre-bundled:

```bash
# 1. Bundle binaries for the current platform into src-tauri/resources/
npm run bundle:binaries           # everything (PHP, MariaDB, phpMyAdmin, Composer)
# or:
npm run bundle:php-only           # just PHP
python3 scripts/bundle-binaries.py --list-versions   # see pinned versions

# 2. Build the installer — Tauri picks up the resources/ dir automatically
npm run tauri:build
```

The bundled binaries live under `src-tauri/resources/bin/<os>-<arch>/<component>-<version>/`. The hybrid resolver finds them automatically — no code changes needed. Users can still override the strategy in Settings → General → Binary strategy.

To rotate the bundled versions, edit `VERSIONS` and `CHECKSUMS` in `scripts/bundle-binaries.py`, then re-run.

### One-shot build (bundle + tauri:build)

```bash
npm run dist
```

---

## First-run flow

1. **App launches**, sees no `settings.json` → redirects to `/wizard`.
2. **Wizard step 1**: choose binary strategy (Auto / PreferSystem / PreferBundled / PreferDownload).
3. **Wizard step 2**: pick PHP, MariaDB, nginx, phpMyAdmin versions.
4. **Wizard step 3**: pick ports (defaults: 8080 / 8443 / 33060 — all unprivileged).
5. **Wizard step 4**: theme + auto-start toggle.
6. **Finish**: validates input, writes `settings.json`, regenerates configs, optionally downloads missing binaries.
7. App redirects to dashboard. Start services, create projects, install WordPress.

---

## Per-project PHP versions

Each project can pin a specific PHP version, independent of the global default. This is useful when you have a legacy WordPress site that needs PHP 8.1 alongside a new Laravel app that needs PHP 8.3.

### How it works

1. **Install multiple PHP versions** — go to Settings → PHP Versions → click "Install" next to each version you need. Each version is downloaded into `~/.insophinia-forage/bin/php-<version>/`.
2. **Set a project's PHP version** — go to Projects → click the PHP dropdown on a project card → pick a version. The override is stored in `~/.insophinia-forage/www/<name>/.forge.json` so it travels with the project folder.
3. **Start services** — the app detects all active PHP versions across projects and spawns one PHP-FPM process per version. Each version listens on its own socket: `run/php-fpm-8.1.sock`, `run/php-fpm-8.3.sock`, etc.
4. **nginx routing** — the generated `nginx.conf` includes a `map $project $php_socket` directive that routes each project's `*.php` requests to the correct socket. Projects without an override fall back to the default socket.

### What happens when you change a project's PHP version

1. The new version is written to `.forge.json`.
2. Configs are regenerated (`nginx.conf` + a new `php-fpm-<version>.conf` if needed).
3. **You need to restart services** for the change to take effect. The UI shows a "restart required" hint.
4. On restart, the app spawns the new PHP-FPM process and nginx picks up the new routing.

### Limitations

- Per-project PHP versions require a separate PHP-FPM process per version. On a 3 GB RAM machine, stick to 2 versions max.
- Switching the global default PHP version (Settings → PHP → PHP version) does NOT migrate existing databases or sessions. It only affects new PHP-FPM spawns.
- MariaDB version switching is not supported — switching requires a data migration.

---

## Conflict resolution: what happens when you already have PHP/MariaDB/nginx installed

This is a common confusion with dev stacks. XAMPP, MAMP, and Laragon all have different behaviors — here's how InsophiniaForage handles it.

### The three sources of binaries

The app can find each component (PHP, nginx, MariaDB, Composer) in three places:

1. **System** — installed via your package manager (`apt install php`, `dnf install nginx`, `brew install mariadb`). Lives in `/usr/bin/`, `/opt/homebrew/bin/`, etc.
2. **Bundled** — shipped inside the installer (under `src-tauri/resources/bin/<platform>-<arch>/`). Only present if you ran `npm run bundle:binaries` before building.
3. **Downloaded** — downloaded on first run into `~/.insophinia-forage/bin/<component>-<version>/`.

### Binary strategy vs. conflict policy

These are two **independent** settings:

| Setting | Where | What it controls |
|---------|-------|------------------|
| **Binary strategy** | Settings → General | Which source to **prefer at runtime** when spawning a service. Order: system → bundled → downloaded (in `Auto` mode). |
| **Conflict policy** | Settings → General | What to do at **install time** when a system install is detected. Should we download a separate copy, use the system one, or replace it? |

### The three conflict policies

#### `Coexist` (default, recommended)
- The app downloads its own copy into `~/.insophinia-forage/bin/` and uses that at runtime.
- The system package is left untouched.
- **Two copies on disk** (wasteful) but zero risk to the system.
- Example: you have `php8.3` from `apt`. The app downloads PHP 8.3.14 into `~/.insophinia-forage/bin/php-8.3.14/` and uses that. Your `apt` PHP is unaffected.

#### `Use system`
- The app does **not** download anything. It uses the system binary as-is.
- Lightest on disk, but:
  - You're responsible for keeping the system package working (`apt upgrade php`).
  - The system PHP may not have all extensions the app needs (e.g. `php-fpm`, `php-mysqlnd`, `php-gd`).
  - The version may not match what the wizard selected — the app shows a warning in this case.
- Best for users who already have a working LAMP stack and want to layer the app's UI on top.

#### `Replace` (not recommended)
- The app runs `apt remove php*` (or equivalent) and then downloads its own copy.
- **Breaks other apps** that depend on the system package.
- The app requires explicit confirmation in the UI before doing this.
- Only use this if you're setting up a fresh machine and want the app to be the single source of truth for PHP/MariaDB.

### What the app does NOT do

- It never silently uninstalls system packages. Even in `Replace` mode, you get a confirmation dialog.
- It never modifies system config files (`/etc/nginx/`, `/etc/php/`, `/etc/mysql/`). All configs are generated under `~/.insophinia-forage/conf/`.
- It never runs as root. Even `Replace` mode asks you to run the uninstall command yourself (the app shows the exact command to copy-paste).

### How to see what was detected

Go to **Settings → Detection tab**. You'll see a table:

| Component | Planned action | System | Bundled | Downloaded | Recommended |
|-----------|---------------|--------|---------|------------|-------------|
| php | Use system | /usr/bin/php v8.3.14 | — | — | 8.3 |
| mariadb | Will download | — | — | — | 11.4 |
| nginx | Use system | /usr/sbin/nginx v1.27.0 | — | — | 1.27 |
| composer | Use downloaded | — | — | ~/.insophinia-forage/bin/composer.phar | stable |
| phpmyadmin | Will download | — | — | — | 5.2.2 |

The "Planned action" column tells you exactly what will happen on next service start. If you see "Will download" for a component that's already on your system, switch the conflict policy to `Use system` to avoid the download.

---

## Version compatibility checks

The app maintains a compatibility matrix and validates it whenever you change a version. There are three severity levels:

- **Error** — blocks the change. The wizard refuses to save; the Settings page disables the save button.
- **Warning** — shows a message but allows the change. The app may not work correctly.
- **Info** — informational, no action needed.

### Current rules

| Component A | Version A | Requires | Component B | Minimum version |
|-------------|-----------|----------|-------------|-----------------|
| phpMyAdmin | 5.x | PHP ≥ 7.2 | PHP | 7.2 |
| phpMyAdmin | 6.x | PHP ≥ 8.1 | PHP | 8.1 |
| phpMyAdmin | any | MariaDB ≥ 10.4 (for utf8mb4 defaults) | MariaDB | 10.4 (warning) |
| WordPress | 6.x | PHP ≥ 7.2 | PHP | 7.2 (error) / 7.4 (warning) |
| MariaDB | 11.x | PHP mysqlnd ≥ 7.4 (for new auth plugin) | PHP | 7.4 (info) |

### Where checks run

1. **Wizard** — when you change versions in step 2, the wizard calls `check_compatibility_for` and shows errors inline. The Continue button is disabled if there are blocking errors.
2. **Settings → PHP / MariaDB / nginx / phpMyAdmin** — when you change a version, the page calls `check_compatibility` and shows warnings at the top of the page.
3. **Settings → Detection tab** — shows all current compatibility issues in one place.
4. **Backend** — the wizard's `save_wizard_input` command runs `validate_settings` before saving and rejects the save if there are blocking issues.

### Adding new rules

Edit `src-tauri/src/core/compatibility.rs` and add a new `validate_<componentA>_<componentB>` function. Each function returns a `Vec<CompatibilityIssue>`. Add it to the `validate_settings` aggregator. The frontend picks up new issues automatically — no UI changes needed.

---

## Security model

InsophiniaForage is a **single-user dev tool**. We assume the user trusts their own machine. That said, we apply these defenses:

- **No root.** All services bind to ports ≥ 1024 and run as the current user.
- **Per-user data dir.** Everything lives under `~/.insophinia-forage/` with `0700` permissions on sensitive subdirs.
- **Settings file is `0600`.** It contains the auto-generated DB password.
- **MariaDB binds to `127.0.0.1` only.** No network exposure.
- **`skip-grant-tables` is used** for the dev DB (matches XAMPP's behavior). This is safe because we only listen on localhost.
- **Project names are validated** — `[a-zA-Z0-9._-]` only, max 64 chars, no leading dot. Prevents path traversal.
- **SQL identifiers are backtick-escaped** before being interpolated into `CREATE DATABASE` / `DROP DATABASE`.
- **No `eval` or string-form SQL** — every query is sent via the `mariadb` client's `-e` flag.
- **Downloads verify SHA-256** when checksums are available (PHP/nginx/MariaDB official sources publish them).
- **CSP is locked down** in `tauri.conf.json`: `default-src 'self'`, no remote scripts.

### What we don't defend against

- An attacker with a shell on your machine. They can read `settings.json` and the DB files. This is true of every dev stack (XAMPP, MAMP, Laragon, Local).
- Malicious WordPress plugins. We install a clean WordPress; what you install after that is on you.

---

## Reliability — no memory leaks, no CPU hogs

The Rust backend is designed to be cheap to keep running in the background:

| Concern | Mitigation |
|---------|------------|
| Orphan processes after crash | Every spawned child is in `ProcessManager.slots`. On window destroy, `stop_all()` runs `kill + wait`. PID files at `~/.insophinia-forage/run/*.pid` enable cross-restart orphan detection. |
| Stdout pipe buffer deadlock | Each child gets two pump threads (stdout, stderr) that drain to the per-service log file. Children never block on a full pipe. |
| Log file unbounded growth | `tracing-appender` rotates daily. A background sweeper deletes logs older than 7 days on startup. |
| Status polling CPU | UI polls every 2 s; each poll is `try_wait()` on each child (cheap kernel call). |
| Memory bloat from large logs | UI requests last 1000 lines max via `tail_log`. We don't keep full log content in memory. |
| Settings file corruption | All writes go through `tmp + rename` (atomic on POSIX). |
| Race on settings | `parking_lot::RwLock<Settings>` — read-heavy, write-rare. |
| Race on process slots | `parking_lot::Mutex<HashMap<...>>`. Lock held briefly. |
| Process group zombies | On Unix we set `process_group(0)` so killing the parent reaps the group. |

---

## Config templates

All four config files are generated from templates embedded in the binary via `include_str!`:

- `src-tauri/src/templates/nginx.conf.template`
- `src-tauri/src/templates/php-fpm.conf.template`
- `src-tauri/src/templates/php.ini.template`
- `src-tauri/src/templates/my.cnf.template`

Templates use `{{placeholder}}` syntax. To change defaults, edit the template + the `render_*` function in `src-tauri/src/core/config_gen.rs`.

---

## Binary sources

Where the bundled/downloaded binaries come from, and which sources are official:

| Component | Windows | Linux | macOS |
|-----------|---------|-------|-------|
| **PHP** | ✅ Official `windows.php.net/downloads/releases/` | ⚠️ `static-php-cli` (third-party, compiles from official PHP source) | ⚠️ `static-php-cli` (third-party) |
| **nginx** | ✅ Official `nginx.org/download/` | ❌ No binary — must use system package via `Use system` conflict policy | ❌ No binary — must use system package |
| **MariaDB** | ✅ Official `archive.mariadb.org/` | ✅ Official `archive.mariadb.org/` | ✅ Official `archive.mariadb.org/` |
| **phpMyAdmin** | ✅ Official `files.phpmyadmin.net/` | ✅ Official `files.phpmyadmin.net/` | ✅ Official `files.phpmyadmin.net/` |
| **Composer** | ✅ Official `getcomposer.org/composer-stable.phar` | ✅ Official `getcomposer.org/composer-stable.phar` | ✅ Official `getcomposer.org/composer-stable.phar` |
| **WordPress** | ✅ Official `wordpress.org/latest.zip` | ✅ Official `wordpress.org/latest.zip` | ✅ Official `wordpress.org/latest.zip` |

### Why are PHP/nginx on Linux/macOS not from official sources?

Because **the PHP and nginx projects don't publish official Linux/macOS binaries**. They ship source code only — the official policy is "your distro's package manager compiles it." So for those two on Linux/macOS, we have **four** choices:

1. **Use the system package via official maintainer repos** (recommended) — the app generates the exact commands to install from `packages.sury.org/php` (Debian/Ubuntu, maintained by Ondřej Surý — the official Debian PHP maintainer), `rpms.remirepo.net` (Fedora, maintained by Remi Collet — the official Fedora PHP maintainer), `nginx.org/packages/...` (official nginx.org apt/yum repo), or Homebrew on macOS. See "Install via package manager" below.

2. **Use the system package via distro's default repo** (default via `Use system` conflict policy) — `apt install php`, `brew install php`, etc. Whatever version your distro ships.

3. **Use a third-party static build** (default via `Coexist` conflict policy) — `static-php-cli` compiles from official PHP source with all extensions, then publishes the binary. Closest to "official" while still being a pre-built binary that doesn't need root.

4. **Compile from source on first run** (advanced) — the app downloads the official source tarball and compiles it locally. Slow (5–10 min) and needs build deps. See "Compiling from source" below.

For nginx on Linux/macOS, options 1, 2, and 4 work. Option 3 (static build) is not available — there's no third-party static nginx build we trust.

### Dynamic version fetching

On app startup (and on every wizard launch), the app queries the official APIs to populate the version dropdowns:

| Component | API | Returns |
|-----------|-----|---------|
| PHP | `https://www.php.net/releases/index.php?json` | All major versions (8.x, 9.x, ...) |
| nginx | `https://nginx.org/en/download.html` (scraped) | Latest 8 versions |
| MariaDB | `https://downloads.mariadb.org/rest-api/mariadb/` | All releases, grouped by major.minor |
| phpMyAdmin | `https://www.phpmyadmin.net/files.json` | Latest 8 versions |
| WordPress | `https://api.wordpress.org/core/version-check/1.7/` | Latest version + "latest" alias |

All fetches are async and time out after 10 seconds. If a fetch fails (no internet, API down), the app falls back to a hardcoded list and shows a warning in the wizard. The user can always type a custom version via the "Custom…" option in every dropdown.

### Adding a new version manually

Every version dropdown has a "Custom…" option that reveals a text input. Type any version string (e.g. `8.4.1`, `11.6.2`, `1.27.5`) and the app will try to download it from the appropriate source. If the version doesn't exist, the download fails with a clear error.

### Install via package manager (recommended for Linux/macOS)

**Where to find it:** Settings → PHP Versions → scroll to "Install via package manager" section.

This is the **best option for most Linux/macOS users**. The app generates the exact shell commands to install PHP/nginx from official package-manager sources maintained by the official distro maintainers:

| OS | PHP source | nginx source | Maintainer |
|----|-----------|--------------|------------|
| **macOS** | `brew install php@8.5` | `brew install nginx` | Homebrew team |
| **Debian/Ubuntu/Mint** | `packages.sury.org/php` | `nginx.org/packages/ubuntu` | Ondřej Surý (official Debian PHP maintainer) + nginx team |
| **Fedora/RHEL** | `rpms.remirepo.net` | `nginx.org/packages/centos` | Remi Collet (official Fedora PHP maintainer) + nginx team |

**Why this is better than the static builds:**
- Binaries are built and signed by **the official distro maintainers** themselves
- Auto-updated via `apt upgrade` / `dnf upgrade` / `brew upgrade`
- All PHP extensions available as separate packages (`php8.5-mysql`, `php8.5-gd`, etc.)
- Properly integrated with system services (systemd, launchd)

**Root access:** apt/dnf commands need `sudo`. The app **never runs `sudo`** — it shows you the exact commands to copy-paste into your terminal. brew on macOS doesn't need sudo.

**The flow:**

1. Click "Install PHP via package manager" (or nginx)
2. Enter the version you want (e.g. `8.5`)
3. The app detects your OS + package manager and generates an ordered list of commands
4. Click "Copy all commands" — paste into your terminal — run them in order
5. Click "I've run these — verify" — the app checks if the binary is now on PATH
6. If verified, the hybrid resolver picks it up automatically on next service start

**Example: Debian/Ubuntu + PHP 8.5**

The app generates these exact commands:

```bash
sudo apt update
sudo apt install -y lsb-release ca-certificates curl
sudo curl -sSLo /tmp/debsuryorg-archive-keyring.deb https://packages.sury.org/debsuryorg-archive-keyring.deb
sudo dpkg -i /tmp/debsuryorg-archive-keyring.deb
sudo tee /etc/apt/sources.list.d/php.sources <<EOF
Types: deb
URIs: https://packages.sury.org/php/
Suites: $(lsb_release -sc)
Components: main
Signed-By: /usr/share/keyrings/debsuryorg-archive-keyring.gpg
EOF
sudo apt update
sudo apt install -y php8.5-fpm php8.5-cli php8.5-mysql php8.5-gd php8.5-mbstring php8.5-xml php8.5-curl php8.5-zip
```

After running them, click "Verify" — the app finds `/usr/sbin/php8.5-fpm` and adds it to the binary cache.

### Compiling from source (Linux/macOS only)

**Where to find it:** Settings → PHP Versions → scroll to "Compile from source" section.

**When to use it:**
- You want PHP/nginx version X but your distro's package manager doesn't have it
- You don't trust third-party static builds
- You're OK with a 5–10 minute build (PHP) or ~1 minute build (nginx) on first run

**Root access — what requires it:**

| Step | Requires root? | Why |
|------|---------------|-----|
| **Install build deps** (gcc, make, libxml2-dev, etc.) | ✅ Yes (one time) | `apt install` writes to `/usr/`, needs root |
| **Compile PHP/nginx itself** | ❌ No | We install to `~/.insophinia-forage/bin/<component>-<version>-source/` (user-writable) |
| **Run the compiled binary** | ❌ No | Same — user-writable path |

**The flow:**

1. Click "Check build deps" — the app scans for `gcc`, `make`, `pkg-config`, `libxml-2.0`, `openssl`, etc.
2. If anything is missing, the app shows the exact command to copy-paste into your terminal:
   ```
   sudo apt update && sudo apt install -y gcc make pkg-config autoconf bison \
       libxml2-dev libsqlite3-dev zlib1g-dev libcurl4-openssl-dev libssl-dev \
       libpng-dev libpcre2-dev
   ```
   The app **never runs `sudo`** itself — you run this once in your terminal.
3. After installing deps, click "Compile PHP from source" — enter the version (e.g. `8.3.14`).
4. A build-progress dialog opens showing live stdout/stderr from `./configure`, `make`, `make install`.
5. The build takes 5–10 minutes for PHP, ~1 minute for nginx. You can dismiss the dialog — the build continues in the background.
6. When done, the compiled binary lands at `~/.insophinia-forage/bin/php-8.3.14-source/bin/php` and is automatically detected by the hybrid resolver.

**What gets compiled (PHP configure flags):**

```
./configure \
  --prefix=~/.insophinia-forage/bin/php-8.3.14-source \
  --enable-fpm \
  --enable-mbstring --enable-xml --enable-bcmath --enable-pcntl \
  --enable-sockets --enable-opcache \
  --with-curl --with-openssl --with-zlib --with-pcre2 \
  --with-mysqli=mysqlnd --with-pdo-mysql=mysqlnd \
  --with-sqlite3 --with-pdo-sqlite --with-gd --with-zip
```

This matches what most distros ship: FPM + the extensions WordPress/Laravel/common PHP apps need. If you need a custom set of extensions, edit `src-tauri/src/core/source_build.rs::compile_php_from_source` and rebuild the app.

**Build cache:** Source tarballs are cached at `~/.insophinia-forage/cache/source/`. If you compile the same version again, it skips the download. The extracted build dir is cleaned up after each successful build to save disk.

### Bundling binaries into the installer

To bundle binaries into the installer (instead of downloading on first run), run `npm run bundle:binaries` before `npm run tauri:build`. The hybrid resolver will pick them up automatically.

---

## Troubleshooting

**`502 Bad Gateway` from nginx**
- PHP-FPM isn't running. Check the Dashboard — is the PHP service green?
- The socket at `~/.insophinia-forage/run/php-fpm.sock` doesn't exist or has wrong permissions. Stop PHP, delete the socket, start PHP again.

**`Port 8080 is already in use`**
- Another process (often a system nginx or Apache) is holding the port. Either stop it or change the port in Settings → General.

**`*.localhost` doesn't resolve**
- We use port-based URLs (`http://project:8080`), not hostname-based. Modern browsers will resolve `project` as a single-label DNS name to `127.0.0.1` on most systems. If yours doesn't, add a line to `/etc/hosts`: `127.0.0.1 myproject`.

**MariaDB won't start**
- Check `~/.insophinia-forage/logs/mariadb/error.log`.
- Most common cause: data dir not initialized. Run `mariadb-install-db --datadir=~/.insophinia-forage/data/mysql` manually, or use the "Reset to defaults" button in Settings.

**App won't quit cleanly**
- Use the tray icon → Quit. Closing the window just hides it (services keep running). On Quit, we send SIGTERM to all children and wait up to 5s each.

---

## Project structure

```
insophinia-forge/
├── package.json                  # SvelteKit + Tauri deps
├── svelte.config.js              # adapter-static, SPA fallback
├── vite.config.ts                # Tauri-aware Vite config
├── tailwind.config.js            # Mac-style design tokens
├── tsconfig.json
├── src/                          # SvelteKit frontend
│   ├── app.html
│   ├── app.css                   # Design system (CSS vars for light/dark)
│   ├── lib/
│   │   ├── api.ts                # Typed wrappers around invoke()
│   │   ├── types.ts              # Mirrors of Rust serde structs
│   │   ├── stores/index.ts       # Reactive stores (services, projects, settings, binaries)
│   │   └── components/
│   │       ├── Sidebar.svelte
│   │       ├── TopBar.svelte
│   │       └── ServiceCard.svelte
│   └── routes/
│       ├── +layout.svelte        # App shell (sidebar + topbar + content)
│       ├── +layout.ts            # SPA mode (no SSR)
│       ├── +page.svelte          # Dashboard
│       ├── wizard/+page.svelte   # First-run wizard
│       ├── projects/+page.svelte
│       ├── databases/+page.svelte
│       ├── wordpress/+page.svelte
│       ├── logs/+page.svelte
│       └── settings/+page.svelte
├── src-tauri/
│   ├── Cargo.toml                # Rust deps
│   ├── tauri.conf.json           # Tauri 2 config
│   ├── build.rs
│   ├── icons/                    # Generated by scripts/generate-icons.py
│   └── src/
│       ├── main.rs               # Thin binary entry
│       ├── lib.rs                # Tauri builder + command registration
│       ├── error.rs              # ForageError + ForageResult
│       ├── tray.rs               # System tray
│       ├── commands/             # Tauri command handlers
│       │   ├── services.rs
│       │   ├── projects.rs
│       │   ├── databases.rs
│       │   ├── wordpress.rs
│       │   ├── composer.rs
│       │   ├── settings.rs
│       │   ├── binaries.rs
│       │   ├── logs.rs
│       │   ├── php_versions.rs
│       │   ├── detection.rs      # Detection report + compatibility commands
│       │   ├── source_build.rs   # Compile-from-source commands
│       │   ├── package_manager.rs # apt/dnf/brew install commands
│       │   └── wizard.rs
│       ├── core/
│       │   ├── mod.rs            # AppState
│       │   ├── paths.rs          # ~/.insophinia-forage/ layout
│       │   ├── settings.rs       # Settings + SettingsManager + ConflictPolicy
│       │   ├── process.rs        # ProcessManager (child lifecycle)
│       │   ├── spawn.rs          # SpawnSpec builders per service + per PHP version
│       │   ├── config_gen.rs     # Template renderer + per-version PHP-FPM configs
│       │   ├── ports.rs          # Port helpers
│       │   ├── logging.rs        # tracing + rotation
│       │   ├── log_watcher.rs    # notify-based real-time log streaming
│       │   ├── php_versions.rs   # Per-project PHP version overrides
│       │   ├── detection.rs      # Detection report (system/bundled/downloaded)
│       │   ├── compatibility.rs  # Version compatibility matrix
│       │   ├── latest_versions.rs # Dynamic version fetcher (official APIs)
│       │   ├── package_manager.rs # Generate apt/dnf/brew install commands
│       │   └── source_build.rs   # Compile PHP/nginx from source (Linux/macOS)
│       ├── binaries/
│       │   ├── mod.rs            # Hybrid resolver
│       │   ├── detect.rs         # System + bundled detection (resource_dir aware)
│       │   └── download.rs       # Download + extract + verify
│       └── templates/
│           ├── nginx.conf.template
│           ├── php-fpm.conf.template
│           ├── php.ini.template
│           └── my.cnf.template
└── scripts/
    ├── generate-icons.py         # Placeholder PNG icons
    ├── generate-icns.py          # macOS ICNS from PNGs
    └── bundle-binaries.py        # Bundle portable binaries into the installer
```

---

## Roadmap

- [x] ~PHP version switcher per project~ (shipped — see "Per-project PHP versions" above)
- [x] ~Real-time log streaming via `notify` crate~ (shipped — replaced 1.5s polling)
- [x] ~Bundled-mode build script~ (shipped — `npm run bundle:binaries`)
- [x] ~Conflict resolution policy~ (shipped — Coexist / UseSystem / Replace)
- [x] ~Detection report~ (shipped — Settings → Detection tab)
- [x] ~Version compatibility matrix~ (shipped — enforced in wizard + Settings)
- [ ] Auto-update via `tauri-plugin-updater`
- [ ] WP-CLI integration for headless WordPress setup
- [ ] Backup/restore: zip a project + DB dump
- [ ] Real-time log streaming coalescing (debounce rapid writes within 50ms)
- [ ] PHP version switcher: per-project PHP INI overrides (memory_limit, etc.)
- [ ] MariaDB version migration tool (dump → upgrade → restore)
- [ ] Detection report: show which PHP extensions are loaded in the system PHP

---

## Project status & next steps

### What this project is

InsophiniaForage is an **experimental exploration** of what a modern, cross-platform PHP dev stack could look like — built with Tauri 2 (Rust) + SvelteKit instead of Electron. The goal was to learn whether the architecture of XAMPP/Laragon/Local could be rebuilt cleanly with current tech, with:

- Low memory (~30–80 MB idle, vs Electron's 200–400 MB)
- Per-user, no-root operation (no system service files modified)
- A Mac-style UI that doesn't look like it's from 2005
- A hybrid binary strategy that respects what the user already has installed

### What's done

The architecture is complete and the frontend builds cleanly. Every major feature is implemented:

- Service lifecycle (start/stop nginx, PHP-FPM, MariaDB with proper child reaping)
- Project manager (folder-per-project under `www/`)
- Database manager (create/drop/SQL via the `mariadb` CLI)
- One-click WordPress installer (download → extract → DB → `wp-config.php`)
- Composer runner
- Real-time log streaming via `notify` (inotify/FSEvents/ReadDirectoryChangesW)
- Per-project PHP version switcher (one PHP-FPM per active version, nginx `map` routing)
- First-run wizard with live compatibility checks
- Dynamic version fetching from official APIs (php.net, nginx.org, mariadb.org, etc.)
- Hybrid binary resolution (system → bundled → downloaded)
- Conflict resolution policy (Coexist / Use system / Replace)
- Compile-from-source pipeline (PHP/nginx, Linux/macOS only)
- Package-manager install command generator (apt sury.org / dnf remirepo / brew)
- Detection report + compatibility matrix
- System tray with Start All / Stop All / Quit
- `tracing` + daily rotation + 7-day sweep
- Mac-style UI with dark mode + responsive layout

### What's NOT done (caveats)

This is **experimental software**. Things that are missing or untested:

- **No CI / automated tests.** The Rust unit tests cover the config renderer and version comparison logic, but there's no integration test that actually spawns nginx/PHP/MariaDB and verifies they answer HTTP requests.
- **No security audit.** The CSP, file permissions, and child-process isolation are designed carefully but have not been reviewed by a security professional.
- **No production usage.** The app has only been run on a handful of dev machines. There are almost certainly bugs in edge cases (port conflicts, version mismatches, OS-specific quirks).
- **No auto-update.** Once installed, the app stays at the version you installed. Updating means re-downloading or rebuilding.
- **Windows is least tested.** The Rust code compiles for Windows, but the per-version PHP-FPM pool config + Unix socket routing assumes Unix. Windows users should use TCP sockets instead (a small change to `php-fpm.conf.template`).
- **The compile-from-source feature** runs `./configure && make && make install` with hardcoded flags. It does not detect what extensions your specific PHP codebase needs.
- **The package-manager install commands** are based on the maintainers' published docs at the time of writing. They will break if the maintainer changes the repo URL, signing key, or package names — and you'll need to edit `src-tauri/src/core/package_manager.rs` to fix them.

### What you should do before using this seriously

1. **Read [`DISCLAIMER`](./DISCLAIMER)** — understand what you're getting into.
2. **Back up** `~/.insophinia-forage/` if you have an older version.
3. **Use a separate user account** if you want isolation. The app's data dir is per-user, so a dedicated `forge` user keeps things separate from your main account.
4. **Don't use it for client work.** Treat it as a learning project.
5. **Verify the configs it generates.** Read `~/.insophinia-forage/conf/nginx.conf`, `php-fpm.conf`, `my.cnf` before starting services.
6. **Read the source.** The whole thing is ~9,000 LOC of Rust + TypeScript. You can read it in an afternoon.

### Where to go from here

If you want to take this further:

- **Add integration tests** — spawn nginx + PHP-FPM, hit `http://test:8080/`, assert `phpinfo()` returns 200. The infrastructure is there (Tauri's test harness), just no tests written.
- **Add Windows TCP socket support** — `php-fpm.conf.template` should use `listen = 127.0.0.1:9000` on Windows instead of a Unix socket. Detect at config-gen time.
- **Add WP-CLI integration** — bundle `wp-cli.phar`, add a "run WP-CLI" command per project.
- **Add backup/restore** — `zip -r project.zip www/<name>/` + `mariadb-dump > project.sql`. Straightforward to add as a new command.
- **Add auto-update** — `tauri-plugin-updater` is one crate away. Just need a signing key + a release pipeline.
- **Harden the security model** — drop `skip-grant-tables` and properly initialize the MariaDB root password; add a sandboxed WebView2 config on Windows; audit the CSP.
- **Add a real test pipeline** — GitHub Actions matrix on Linux + macOS + Windows, running `cargo test` + `npm run check` + `npm run tauri:build`.

### Final word

This was a fun exploration. If you build on it, please share what you make — and remember: **back up your data, don't trust it for production, and use at your own risk.**

---

## License

MIT. See `LICENSE`. By using this software you also accept the terms in [`DISCLAIMER`](./DISCLAIMER).
