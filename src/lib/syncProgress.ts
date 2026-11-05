export function formatDuration(milliseconds: number): string {
  const safeMilliseconds = Math.max(0, milliseconds);
  if (safeMilliseconds < 60_000) {
    return `${(safeMilliseconds / 1000).toFixed(1)} s`;
  }
  const totalSeconds = Math.round(safeMilliseconds / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, '0');
  return `${minutes}:${seconds}`;
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const unit = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** unit;
  const precision = unit === 0 || value >= 10 ? 0 : 1;
  return `${value.toFixed(precision)} ${units[unit]}`;
}
