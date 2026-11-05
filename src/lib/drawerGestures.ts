export const DRAWER_EDGE_WIDTH = 24;
export const DRAWER_OPEN_DISTANCE = 60;

export function isHorizontalSwipe(deltaX: number, deltaY: number): boolean {
  return Math.abs(deltaX) > 8 && Math.abs(deltaX) > Math.abs(deltaY) * 1.2;
}

export function drawerOffset(deltaX: number, width: number, opening: boolean): number {
  return opening ? Math.min(0, deltaX - width) : Math.max(-width, Math.min(0, deltaX));
}

export function shouldToggleDrawer(deltaX: number, opening: boolean): boolean {
  return opening ? deltaX >= DRAWER_OPEN_DISTANCE : deltaX <= -DRAWER_OPEN_DISTANCE;
}
