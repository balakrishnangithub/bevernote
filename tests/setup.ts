import { afterEach } from 'vitest';

afterEach(() => {
  document.body.innerHTML = '';
});

if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => undefined;
}
