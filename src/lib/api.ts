// Typed wrappers around `invoke()`. All Tauri calls go through this file
// so we have one place to audit, log, and version the API surface.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AvailableVersions,
  BinaryStatus,
  BuildDepsReport,
  ComposerResult,
  CompatibilityIssue,
  Database,
  DetectionReport,
  InstallPlan,
  PackageManager,
  Project,
  ProjectKind,
  PhpVersionInfo,
  PlannedAction,
  ServiceStatus,
  Settings,
  WordPressInstallResult,
  WizardInput,
  WizardResult
} from './types';

// ---- Services --------------------------------------------------------------

export const startService = (name: string): Promise<void> =>
  invoke('start_service', { name });

export const stopService = (name: string): Promise<void> =>
  invoke('stop_service', { name });

export const serviceStatus = (): Promise<ServiceStatus[]> =>
  invoke('service_status');

export const isServiceRunning = (name: string): Promise<boolean> =>
  invoke('is_service_running', { name });

// ---- Projects --------------------------------------------------------------

export const listProjects = (): Promise<Project[]> =>
  invoke('list_projects');

export const createProject = (name: string, kind: ProjectKind): Promise<Project> =>
  invoke('create_project', { name, kind });

export const deleteProject = (name: string): Promise<void> =>
  invoke('delete_project', { name });

export const openProjectInBrowser = (name: string): Promise<void> =>
  invoke('open_project_in_browser', { name });

export const openProjectInFileManager = (name: string): Promise<void> =>
  invoke('open_project_in_file_manager', { name });

// ---- Databases -------------------------------------------------------------

export const listDatabases = (): Promise<Database[]> =>
  invoke('list_databases');

export const createDatabase = (name: string): Promise<void> =>
  invoke('create_database', { name });

export const dropDatabase = (name: string): Promise<void> =>
  invoke('drop_database', { name });

export const queryDatabase = (db: string, sql: string): Promise<string> =>
  invoke('query_database', { db, sql });

export const openPhpmyadmin = (): Promise<void> =>
  invoke('open_phpmyadmin');

// ---- WordPress -------------------------------------------------------------

export const installWordPress = (
  projectName: string,
  siteTitle?: string,
  adminUser?: string,
  adminPassword?: string,
  adminEmail?: string
): Promise<WordPressInstallResult> =>
  invoke('install_wordpress', {
    projectName,
    siteTitle,
    adminUser,
    adminPassword,
    adminEmail
  });

// ---- Composer --------------------------------------------------------------

export const composerRun = (projectName: string, args: string[]): Promise<ComposerResult> =>
  invoke('composer_run', { projectName, args });

// ---- Settings --------------------------------------------------------------

export const getSettings = (): Promise<Settings> =>
  invoke('get_settings');

export const updateSettings = (key: string, value: unknown): Promise<Settings> =>
  invoke('update_settings', { key, value });

export const resetSettings = (): Promise<Settings> =>
  invoke('reset_settings');

export const isInitialized = (): Promise<boolean> =>
  invoke('is_initialized');

// ---- Binaries --------------------------------------------------------------

export const listBinaries = (): Promise<BinaryStatus[]> =>
  invoke('list_binaries');

export const downloadBinary = (name: string, version: string): Promise<void> =>
  invoke('download_binary', { name, version });

export const refreshBinaries = (): Promise<void> =>
  invoke('refresh_binaries');

// ---- Logs ------------------------------------------------------------------

export const tailLog = (service: string, n?: number): Promise<string[]> =>
  invoke('tail_log', { service, n });

export const subscribeLog = (service: string): Promise<void> =>
  invoke('subscribe_log', { service });

export const unsubscribeLog = (service: string): Promise<void> =>
  invoke('unsubscribe_log', { service });

// ---- Wizard ----------------------------------------------------------------

export const validateWizardInput = (input: WizardInput): Promise<WizardResult> =>
  invoke('validate_wizard_input', { input });

export const saveWizardInput = (input: WizardInput): Promise<WizardResult> =>
  invoke('save_wizard_input', { input });

// ---- PHP versions ----------------------------------------------------------

export const listPhpVersions = (): Promise<PhpVersionInfo[]> =>
  invoke('list_php_versions');

export const getProjectPhpVersion = (projectName: string): Promise<string | null> =>
  invoke('get_project_php_version', { projectName });

export const setProjectPhpVersion = (
  projectName: string,
  version: string | null
): Promise<void> =>
  invoke('set_project_php_version', { projectName, version });

export const listProjectPhpOverrides = (): Promise<Record<string, string | null>> =>
  invoke('list_project_php_overrides');

export const installPhpVersion = (version: string): Promise<void> =>
  invoke('install_php_version', { version });

// ---- Detection + Compatibility ---------------------------------------------

export const getDetectionReport = (): Promise<DetectionReport> =>
  invoke('get_detection_report');

export const checkCompatibility = (): Promise<CompatibilityIssue[]> =>
  invoke('check_compatibility');

export const checkCompatibilityFor = (
  php: string,
  mariadb: string,
  nginx: string,
  phpmyadmin: string,
  wordpress: string
): Promise<CompatibilityIssue[]> =>
  invoke('check_compatibility_for', { php, mariadb, nginx, phpmyadmin, wordpress });

export const setConflictPolicy = (policy: string): Promise<void> =>
  invoke('set_conflict_policy', { policy });

export const wouldReplaceSystemInstall = (component: string): Promise<boolean> =>
  invoke('would_replace_system_install', { component });

export const actionLabel = (action: PlannedAction): Promise<string> =>
  invoke('action_label', { action });

export const fetchAvailableVersions = (): Promise<AvailableVersions> =>
  invoke('fetch_available_versions');

// ---- Source compilation (Linux/macOS only) ---------------------------------

export const checkPhpBuildDeps = (): Promise<BuildDepsReport> =>
  invoke('check_php_build_deps');

export const isSourceCompileSupported = (): Promise<boolean> =>
  invoke('is_source_compile_supported');

export const compilePhpFromSource = (version: string): Promise<string> =>
  invoke('compile_php_from_source', { version });

export const compileNginxFromSource = (version: string): Promise<string> =>
  invoke('compile_nginx_from_source', { version });

// ---- Package manager install (apt/dnf/brew) --------------------------------

export const detectPackageManager = (): Promise<PackageManager> =>
  invoke('detect_package_manager');

export const getInstallPlan = (component: string, version: string): Promise<InstallPlan> =>
  invoke('get_install_plan', { component, version });

export const verifySystemInstall = (component: string, version: string): Promise<string | null> =>
  invoke('verify_system_install', { component, version });

// ---- Event listeners -------------------------------------------------------

export function onServiceStatusChanged(cb: (name: string) => void): Promise<UnlistenFn> {
  return listen<string>('service://status-changed', (e) => cb(e.payload));
}

export function onLogLine(
  cb: (p: { service: string; line: string }) => void
): Promise<UnlistenFn> {
  return listen<{ service: string; line: string }>(
    'service://log-line',
    (e) => cb(e.payload)
  );
}

export function onBuildProgress(
  cb: (p: { stage: string; line: string }) => void
): Promise<UnlistenFn> {
  return listen<{ stage: string; line: string }>(
    'build://progress',
    (e) => cb(e.payload)
  );
}

export function onDownloadProgress(
  cb: (p: { name: string; done: number; total: number }) => void
): Promise<UnlistenFn> {
  return listen<{ name: string; done: number; total: number }>(
    'binary://download-progress',
    (e) => cb(e.payload)
  );
}

export function onWizardCompleted(cb: () => void): Promise<UnlistenFn> {
  return listen('wizard//completed', () => cb());
}
