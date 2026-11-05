import { describe, expect, it } from 'vitest';

import { formatBytes, formatDuration } from './syncProgress';

describe('sync progress formatting', () => {
  it('shows sub-second precision and minute-based elapsed time', () => {
    expect(formatDuration(400)).toBe('0.4 s');
    expect(formatDuration(12_345)).toBe('12.3 s');
    expect(formatDuration(65_000)).toBe('1:05');
    expect(formatDuration(-1)).toBe('0.0 s');
  });

  it('uses compact binary units for transferred bytes', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(12 * 1024 * 1024)).toBe('12 MB');
    expect(formatBytes(Number.NaN)).toBe('0 B');
  });
});
