export const CONFIG_VERSION = 1;
export const DEFAULT_FILE_FONT_SIZE = 18;
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
  showHidden?: {
    left?: boolean | null;
    right?: boolean | null;
  } | null;
}

export interface NormalizedConfig {
  leftPath: string | null;
  rightPath: string | null;
  columns: number[] | null;
  fileFontSize: number;
  showHidden: { left: boolean; right: boolean };
}

export function normalizeFontSize(value: unknown): number {
  const size = typeof value === 'number' ? Math.round(value) : NaN;
  if (!Number.isFinite(size)) return DEFAULT_FILE_FONT_SIZE;
  return Math.min(MAX_FILE_FONT_SIZE, Math.max(MIN_FILE_FONT_SIZE, size));
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
  const hidden =
    typeof config.showHidden === 'object' && config.showHidden !== null
      ? (config.showHidden as Record<string, unknown>)
      : {};
  return {
    leftPath: normalizePath(config.leftPath),
    rightPath: normalizePath(config.rightPath),
    columns: normalizeSharedColumns(config.columnWeights),
    fileFontSize: normalizeFontSize(config.fileFontSize),
    showHidden: {
      left: hidden.left === true,
      right: hidden.right === true,
    },
  };
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
    showHidden: {
      left: normalized.showHidden.left,
      right: normalized.showHidden.right,
    },
  };
}
