import type { SftpProfile } from '../filesystem/types';
export const CONFIG_VERSION = 1;
export const DEFAULT_FILE_FONT_SIZE = 18;
export const DEFAULT_COMPRESSION_LEVEL = 6;
export const MIN_FILE_FONT_SIZE = 12;
export const MAX_FILE_FONT_SIZE = 24;
export const COLUMN_COUNT = 4;
export const MIN_COLUMN_WEIGHT = 20;
export const MAX_COLUMN_WEIGHT = 2000;

export interface AppConfig {
  version: number;
  leftPath?: string | null;
  rightPath?: string | null;
  columnWeights?:
    | {
        left?: number[] | null;
        right?: number[] | null;
      }
    | number[]
    | null;
  fileFontSize?: number | null;
  // Legacy files stored a per-panel object; only boolean true enables it now.
  showHidden?:
    boolean | { left?: boolean | null; right?: boolean | null } | null;
  showFunctionBar?: boolean | null;
  pinnedDirectories?: string[];
  pinnedDirectoryNames?: Record<string, string>;
  compressionLevel?: number;
  sftpProfiles?: SftpProfile[];
  editor?: string | null;
}

export interface NormalizedConfig {
  leftPath: string | null;
  rightPath: string | null;
  columns: number[] | null;
  fileFontSize: number;
  showHidden: boolean;
  showFunctionBar: boolean;
  pinnedDirectories?: string[];
  pinnedDirectoryNames?: Record<string, string>;
  compressionLevel?: number;
  sftpProfiles?: SftpProfile[];
  editor?: string | null;
}

export function normalizeFontSize(value: unknown): number {
  const size = typeof value === 'number' ? Math.round(value) : NaN;
  if (!Number.isFinite(size)) return DEFAULT_FILE_FONT_SIZE;
  return Math.min(MAX_FILE_FONT_SIZE, Math.max(MIN_FILE_FONT_SIZE, size));
}

export function normalizeCompressionLevel(value: unknown): number {
  return value === 1 || value === 9 ? value : DEFAULT_COMPRESSION_LEVEL;
}

export function normalizeColumnWeights(value: unknown): number[] | null {
  if (!Array.isArray(value) || value.length !== COLUMN_COUNT) return null;
  const weights = value.map((weight) =>
    typeof weight === 'number' ? Math.round(weight * 10) / 10 : NaN,
  );
  if (
    !weights.every(
      (weight) =>
        Number.isFinite(weight) &&
        weight >= MIN_COLUMN_WEIGHT &&
        weight <= MAX_COLUMN_WEIGHT,
    )
  )
    return null;
  return weights;
}

function normalizePath(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const path = value.trim();
  return path ? path : null;
}

export function normalizeConfig(raw: unknown): NormalizedConfig {
  const config =
    typeof raw === 'object' && raw !== null
      ? (raw as Record<string, unknown>)
      : {};
  return {
    leftPath: normalizePath(config.leftPath),
    rightPath: normalizePath(config.rightPath),
    columns: normalizeSharedColumns(config.columnWeights),
    fileFontSize: normalizeFontSize(config.fileFontSize),
    // Legacy files stored a per-panel {left, right} object; only an explicit
    // boolean true enables hidden files now.
    showHidden: config.showHidden === true,
    showFunctionBar: config.showFunctionBar !== false,
    pinnedDirectories: normalizePins(config.pinnedDirectories),
    pinnedDirectoryNames: normalizePinNames(config.pinnedDirectoryNames),
    compressionLevel: normalizeCompressionLevel(config.compressionLevel),
    sftpProfiles: normalizeSftpProfiles(config.sftpProfiles),
    editor: normalizePath(config.editor),
  };
}

export function normalizePins(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return [
    ...new Set(value.map(normalizePath).filter((p): p is string => p !== null)),
  ];
}
function normalizeSftpProfiles(value: unknown): SftpProfile[] {
  if (!Array.isArray(value)) return [];
  const profiles: SftpProfile[] = [];
  for (const item of value.slice(0, 100)) {
    if (!item || typeof item !== 'object') continue;
    const { id, name, host, port, username, directory, keyPath } = item;
    if (
      typeof id !== 'string' ||
      !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        id,
      ) ||
      typeof name !== 'string' ||
      !name.trim() ||
      typeof host !== 'string' ||
      !host.trim() ||
      typeof username !== 'string' ||
      !username.trim() ||
      !Number.isInteger(port) ||
      port < 1 ||
      port > 65535 ||
      (directory != null && typeof directory !== 'string') ||
      (keyPath != null && typeof keyPath !== 'string')
    )
      continue;
    if (profiles.some((profile) => profile.id === id)) continue;
    profiles.push({
      id,
      name: name.trim(),
      host: host.trim(),
      port,
      username: username.trim(),
      directory: directory ?? '',
      keyPath: keyPath ?? null,
    });
  }
  return profiles;
}

function normalizePinNames(value: unknown): Record<string, string> {
  if (typeof value !== 'object' || value === null || Array.isArray(value))
    return {};
  return Object.fromEntries(
    Object.entries(value)
      .filter(
        ([path, name]) =>
          path.trim() && typeof name === 'string' && name.trim(),
      )
      .map(([path, name]) => [path, (name as string).trim()]),
  );
}

function normalizeSharedColumns(value: unknown): number[] | null {
  if (Array.isArray(value)) return normalizeColumnWeights(value);
  if (typeof value === 'object' && value !== null) {
    const weights = value as Record<string, unknown>;
    return (
      normalizeColumnWeights(weights.left) ??
      normalizeColumnWeights(weights.right)
    );
  }
  return null;
}

export function buildSavePayload(normalized: NormalizedConfig): AppConfig {
  return {
    version: CONFIG_VERSION,
    leftPath: normalized.leftPath,
    rightPath: normalized.rightPath,
    columnWeights: {
      left: normalized.columns,
      right: normalized.columns,
    },
    fileFontSize: normalized.fileFontSize,
    showHidden: normalized.showHidden,
    showFunctionBar: normalized.showFunctionBar,
    pinnedDirectories: normalizePins(normalized.pinnedDirectories),
    pinnedDirectoryNames: normalizePinNames(normalized.pinnedDirectoryNames),
    compressionLevel: normalizeCompressionLevel(normalized.compressionLevel),
    sftpProfiles: normalizeSftpProfiles(normalized.sftpProfiles),
    editor: normalizePath(normalized.editor),
  };
}
