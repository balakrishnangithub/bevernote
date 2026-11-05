import { describe, expect, it } from 'vitest';

import { drawerOffset, isHorizontalSwipe, shouldToggleDrawer } from './drawerGestures';

describe('touch drawer gestures', () => {
  it('accepts deliberate horizontal swipes', () => {
    expect(isHorizontalSwipe(70, 8)).toBe(true);
    expect(shouldToggleDrawer(70, true)).toBe(true);
    expect(shouldToggleDrawer(-70, false)).toBe(true);
  });

  it('rejects vertical scrolling and short taps', () => {
    expect(isHorizontalSwipe(12, 70)).toBe(false);
    expect(isHorizontalSwipe(2, 1)).toBe(false);
    expect(shouldToggleDrawer(35, true)).toBe(false);
    expect(shouldToggleDrawer(-35, false)).toBe(false);
  });

  it('clamps drag positions to the drawer width', () => {
    expect(drawerOffset(20, 300, true)).toBe(-280);
    expect(drawerOffset(800, 300, true)).toBe(0);
    expect(drawerOffset(-800, 300, false)).toBe(-300);
    expect(drawerOffset(50, 300, false)).toBe(0);
  });
});
