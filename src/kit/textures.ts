// Canvas-drawn sprite textures for the kit's effects. Each one has an ink or white rim so it reads on both
// dark and light desktops. Need a DOM canvas: callers skip them outside the browser (unit tests).
import * as THREE from 'three';
import { ANGER_RED, GOLD, INK } from './palette';

const css = (color: number): string => `#${color.toString(16).padStart(6, '0')}`;

function canvasTexture(size: number, draw: (ctx: CanvasRenderingContext2D, c: number) => void): THREE.CanvasTexture {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = size;
  draw(canvas.getContext('2d')!, size / 2);
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

/** A letter (the Zzz) in the character's shade colour with a soft white rim (the old CSS text-shadow). */
export function glyphTexture(glyph: string, color: number): THREE.CanvasTexture {
  const size = 64;
  return canvasTexture(size, (ctx, c) => {
    ctx.font = `600 ${size * 0.8}px 'Segoe UI', system-ui, sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.lineJoin = 'round';
    ctx.lineWidth = 6;
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.85)';
    ctx.strokeText(glyph, c, c);
    ctx.fillStyle = css(color);
    ctx.fillText(glyph, c, c);
  });
}

/** The anime angry mark: four bent strokes whose corners point at the centre, red with a white rim. */
export function angerTexture(): THREE.CanvasTexture {
  const gap = 13;
  const reach = 50;
  return canvasTexture(128, (ctx, c) => {
    ctx.beginPath();
    for (const [sx, sy] of [[1, 1], [1, -1], [-1, 1], [-1, -1]]) {
      ctx.moveTo(c + sx * gap, c + sy * reach);
      ctx.quadraticCurveTo(c + sx * gap * 0.55, c + sy * gap * 0.55, c + sx * reach, c + sy * gap);
    }
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.lineWidth = 24;
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.92)';
    ctx.stroke();
    ctx.lineWidth = 13;
    ctx.strokeStyle = ANGER_RED;
    ctx.stroke();
  });
}

/** A five-pointed star in gold with an ink rim: the "seeing stars" after a hit. */
export function starTexture(): THREE.CanvasTexture {
  return canvasTexture(96, (ctx, c) => {
    ctx.beginPath();
    for (let i = 0; i < 10; i++) {
      const r = i % 2 === 0 ? 40 : 17;
      const a = -Math.PI / 2 + (i * Math.PI) / 5;
      ctx.lineTo(c + Math.cos(a) * r, c + Math.sin(a) * r);
    }
    ctx.closePath();
    ctx.lineJoin = 'round';
    ctx.lineWidth = 7;
    ctx.strokeStyle = css(INK);
    ctx.stroke();
    ctx.fillStyle = css(GOLD);
    ctx.fill();
  });
}

/** The comic impact burst at the point of the hit: a white jagged star with a gold core. */
export function burstTexture(): THREE.CanvasTexture {
  const spikes = 12;
  return canvasTexture(128, (ctx, c) => {
    const shape = (outer: number, inner: number) => {
      ctx.beginPath();
      for (let i = 0; i < spikes * 2; i++) {
        const r = i % 2 === 0 ? outer : inner;
        const a = (i * Math.PI) / spikes;
        ctx.lineTo(c + Math.cos(a) * r, c + Math.sin(a) * r);
      }
      ctx.closePath();
    };
    shape(60, 30);
    ctx.lineJoin = 'round';
    ctx.lineWidth = 6;
    ctx.strokeStyle = css(INK);
    ctx.stroke();
    ctx.fillStyle = '#ffffff';
    ctx.fill();
    shape(34, 18);
    ctx.fillStyle = css(GOLD);
    ctx.fill();
  });
}
