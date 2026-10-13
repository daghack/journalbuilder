import { describe, expect, it } from 'vitest';
import { explain } from './render.service';

describe('explain', () => {
  it('says what to do when the browser offers no WebGPU', () => {
    const message = explain('Request for adapter failed: No suitable graphics adapter found; webgpu found no adapters');
    expect(message).toContain('quit and reopen');
  });

  it('suggests retrying after the GPU is lost', () => {
    expect(explain('the GPU was lost (Unknown): device lost')).toContain('smaller scale');
  });

  it('leaves its own messages and cancellations alone', () => {
    expect(explain('cancelled')).toBe('cancelled');
    expect(explain('Export cancelled.')).toBe('Export cancelled.');
    const once = explain('the renderer crashed: oops');
    expect(once).toBe("Darkly couldn't render: the renderer crashed: oops");
    expect(explain(once)).toBe(once);
  });
});
