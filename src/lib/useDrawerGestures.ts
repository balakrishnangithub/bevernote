import { useRef } from 'react';
import type { PointerEvent } from 'react';

import { DRAWER_EDGE_WIDTH, drawerOffset, isHorizontalSwipe, shouldToggleDrawer } from './drawerGestures';

interface Gesture {
  pointerId: number;
  startX: number;
  startY: number;
  width: number;
  opening: boolean;
  decided: boolean;
}

/** Touch-only, horizontal drawer gestures; vertical scrolling stays native. */
export function useDrawerGestures({
  open,
  blocked,
  onOpen,
  onClose,
}: {
  open: boolean;
  blocked: boolean;
  onOpen: () => void;
  onClose: () => void;
}) {
  const gesture = useRef<Gesture | null>(null);

  const drawer = (event: PointerEvent<HTMLDivElement>) =>
    event.currentTarget.querySelector<HTMLElement>('.sidebar');

  const finish = (event: PointerEvent<HTMLDivElement>, cancelled = false) => {
    const current = gesture.current;
    if (!current || current.pointerId !== event.pointerId) return;
    const deltaX = event.clientX - current.startX;
    const element = drawer(event);
    if (element) {
      element.style.transition = '';
      element.style.transform = '';
    }
    if (!cancelled && current.decided && shouldToggleDrawer(deltaX, current.opening)) {
      if (current.opening) onOpen();
      else onClose();
    }
    if (current.decided && event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    gesture.current = null;
  };

  return {
    onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
      if (event.pointerType !== 'touch' || blocked || !window.matchMedia('(max-width: 760px)').matches)
        return;
      const element = drawer(event);
      if (!element) return;
      const width = element.getBoundingClientRect().width;
      if ((!open && event.clientX > DRAWER_EDGE_WIDTH) || (open && event.clientX > width)) return;
      gesture.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        width,
        opening: !open,
        decided: false,
      };
    },
    onPointerMove: (event: PointerEvent<HTMLDivElement>) => {
      const current = gesture.current;
      if (!current || current.pointerId !== event.pointerId) return;
      const deltaX = event.clientX - current.startX;
      const deltaY = event.clientY - current.startY;
      if (!current.decided) {
        if (Math.abs(deltaY) > 8 && Math.abs(deltaY) > Math.abs(deltaX)) {
          gesture.current = null;
          return;
        }
        if (!isHorizontalSwipe(deltaX, deltaY) || (current.opening && deltaX < 0)) return;
        current.decided = true;
        event.currentTarget.setPointerCapture(event.pointerId);
        const element = drawer(event);
        if (element) element.style.transition = 'none';
      }
      event.preventDefault();
      const element = drawer(event);
      if (element)
        element.style.transform = `translateX(${drawerOffset(deltaX, current.width, current.opening)}px)`;
    },
    onPointerUp: (event: PointerEvent<HTMLDivElement>) => finish(event),
    onPointerCancel: (event: PointerEvent<HTMLDivElement>) => finish(event, true),
  };
}
