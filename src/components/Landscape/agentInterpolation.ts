// N1-1 — nội suy tick ↔ frame (living-world, 2026-08-25).
//
// Renderer chạy theo frame rate (30/60/144 Hz) còn simulation chạy theo tick.
// Giữ hai payload liên tiếp và nội suy giữa chúng theo `alpha` (0 = payload cũ,
// 1 = payload mới) khiến chuyển động mượt ở mọi frame rate thay vì nhảy từng
// tick (AN-S02). Hai hàm này là thuần-tuýy để test được độc lập với Pixi.
//
// Quy ước khớp với test hợp đồng `tests/frontend/livingWorldInterpolation.test.ts`:

export interface Vec3Like {
  x: number;
  y: number;
  z: number;
}

/** Kẹp `alpha` vào [0, 1] — không ngoại suy quá một tick (N1-2). */
function clamp01(alpha: number): number {
  return Math.min(1, Math.max(0, alpha));
}

/** Nội suy tuyến tính từng thành phần giữa hai điểm, alpha kẹp trong [0, 1]. */
export function lerpVec3<T extends Vec3Like>(a: T, b: T, alpha: number): Vec3Like {
  const t = clamp01(alpha);
  return {
    x: a.x + (b.x - a.x) * t,
    y: a.y + (b.y - a.y) * t,
    z: a.z + (b.z - a.z) * t,
  };
}

/** Góc (độ). Đưa hiệu số về (-180, 180] để luôn đi theo cung ngắn, và chuẩn
 *  hóa kết quả về [0, 360). */
export function lerpAngle(fromDeg: number, toDeg: number, alpha: number): number {
  const t = clamp01(alpha);
  let delta = (toDeg - fromDeg) % 360;
  if (delta > 180) delta -= 360;
  if (delta <= -180) delta += 360;
  return (((fromDeg + delta * t) % 360) + 360) % 360;
}
