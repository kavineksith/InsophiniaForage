// Shared types between Rust backend and Svelte frontend. These mirror
// the `#[derive(Serialize)]` structs in src-tauri/src/commands/*.rs.

export type BinarySource = 'bundled' | 'downloaded' | 'system';

export interface ResolvedBinary {
  name: string;
  path: string;
  source: BinarySource;
  version: string | null;
}

export interface Binaries {
  php: ResolvedBinary | null;
  nginx: ResolvedBinary | null;
  mariadb: ResolvedBinary | null;
  composer: ResolvedBinary | null;
}

export interface ServiceStatus {
  name: string;
  running: boolean;
  pid: number | null;
  uptime_seconds: number;
  exit_code: number | null;
  log_file: string;
  command: string;
}

export type ProjectKind = 'plain' | 'word_press' | 'laravel' | 'php_my_admin' | 'custom';

export interface Project {
  name: string;
  path: string;
  kind: ProjectKind;
  url: string;
  size_bytes: number;
  last_modified: string | null;
}

export interface Database {
  name: string;
  tables_count: number;
  size_bytes: number;
}

export interface WordPressInstallResult {
  project_name: string;
  url: string;
  admin_url: string;
  database_name: string;
  database_user: string;
}

export interface ComposerResult {
  exit_code: number;
  stdout: string;
  stderr: string;
}

export type BinaryStrategy = 'auto' | 'prefer_system' | 'prefer_bundled' | 'prefer_download';
export type ConflictPolicy = 'coexist' | 'use_system' | 'replace';
export type Theme = 'light' | 'dark' | 'auto';

export interface Settings {
  version: number;
  ports: {
    nginx_http: number;
    nginx_https: number;
    mariadb: number;
  };
  versions: {
    php: string;
    mariadb: string;
    nginx: string;
    phpmyadmin: string;
    wordpress: string;
  };
  php: {
    memory_limit: string;
    upload_max_filesize: string;
    post_max_size: string;
    max_execution_time: number;
    display_errors: boolean;
  };
  mariadb: {
    innodb_buffer_pool_size: string;
    max_connections: number;
    root_password: string;
    app_user: string;
    app_password: string;
  };
  nginx: {
    client_max_body_size: string;
    worker_processes: string;
  };
  binary_strategy: BinaryStrategy;
  conflict_policy: ConflictPolicy;
  auto_start_services: boolean;
  open_browser_on_start: boolean;
  theme: Theme;
  created_at: string;
}

export interface BinaryStatus {
  name: string;
  resolved: boolean;
  path: string | null;
  source: string | null;
  version: string | null;
  download_url: string | null;
  available_versions: string[];
}

export interface WizardInput {
  versions: {
    php: string;
    mariadb: string;
    nginx: string;
    phpmyadmin: string;
    wordpress: string;
  };
  ports: {
    nginx_http: number;
    nginx_https: number;
    mariadb: number;
  };
  binary_strategy: string;
  theme: string;
  auto_start_services: boolean;
  download_binaries: boolean;
}

export interface WizardResult {
  success: boolean;
  errors: string[];
  warnings: string[];
}

export interface PhpVersionInfo {
  version: string;
  installed: boolean;
  path: string | null;
  source: string | null;
}

// ---- Detection + Compatibility ---------------------------------------------

export type PlannedAction =
  | 'use_system'
  | 'use_bundled'
  | 'use_downloaded'
  | 'download'
  | 'nothing'
  | 'skip';

export interface DetectedInstall {
  path: string;
  version: string | null;
  size_bytes: number;
}

export interface DetectionRow {
  component: string;
  action: PlannedAction;
  system: DetectedInstall | null;
  bundled: DetectedInstall | null;
  downloaded: DetectedInstall | null;
  recommended_version: string;
  warnings: string[];
}

export type Severity = 'error' | 'warning' | 'info';

export interface CompatibilityIssue {
  severity: Severity;
  component_a: string;
  version_a: string;
  component_b: string;
  version_b: string;
  message: string;
}

export interface DetectionReport {
  rows: DetectionRow[];
  pending_downloads: string[];
  estimated_download_bytes: number;
  conflict_policy: ConflictPolicy;
  compatibility_issues: CompatibilityIssue[];
}

// ---- Available versions (fetched from official sources) --------------------

export interface AvailableVersions {
  php: string[];
  nginx: string[];
  mariadb: string[];
  phpmyadmin: string[];
  wordpress: string[];
  fetched: boolean;
  error: string | null;
}

// ---- Source compilation (Linux/macOS only) ---------------------------------

export interface BuildDepsReport {
  all_present: boolean;
  missing_tools: string[];
  missing_libs: string[];
  install_command: string;
  package_manager: string;
}

// ---- Package manager install (apt/dnf/brew) --------------------------------

export type PackageManager = 'apt' | 'dnf' | 'brew' | 'unknown';

export interface ShellStep {
  command: string;
  description: string;
  requires_sudo: boolean;
}

export interface InstallPlan {
  component: string;
  version: string;
  package_manager: PackageManager;
  steps: ShellStep[];
  expected_path: string;
  notes: string[];
}
