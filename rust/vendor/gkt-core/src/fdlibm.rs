//! Java `StrictMath`（fdlibm 5.3）逐位移植：sin/cos/atan/atan2/toDegrees/toRadians。
//!
//! 规范来源：本机 JDK21 的 `java.lang.FdLibm`（Sun fdlibm 5.3 的 Java 移植）。
//! oracle 引擎运行时调用 `Math.*`；JDK21 中 `Math.sin/cos/atan2` 为 `@IntrinsicCandidate`，
//! 默认实现委托 StrictMath，C2 intrinsic 在 double 级有少量 1-ulp 差异
//! （实测 458957 条语料中 sin/cos 共 3643 条，atan2/toDegrees/toRadians 0 条），
//! 引擎所有消费点都 `(float)` 收敛，实测无差异 —— 故以 StrictMath 为唯一规范（AGENTS §4.8）。
//! 校准语料：`fixtures/math/math_corpus.bin`（Java StrictMath 生成，逐位比对）。

// 与 fdlibm/Java 源码逐行对应：`x - x` 生成 NaN、嵌套 if、计数循环等写法看似可简化，
// 但为保持与规范一致的形状而原样保留。
#![allow(clippy::eq_op, clippy::collapsible_if, clippy::explicit_counter_loop)]

use crate::fdlibm_consts::*;

const EXP_BITS: i32 = 0x7ff0_0000;
const EXP_SIGNIF_BITS: i32 = 0x7fff_ffff;

#[inline(always)]
fn hi(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

#[inline(always)]
fn lo(x: f64) -> i32 {
    x.to_bits() as i32
}

/// Java `__HI(x, high)`
#[inline(always)]
fn with_hi(x: f64, high: i32) -> f64 {
    f64::from_bits((x.to_bits() & 0xffff_ffff) | ((high as u32 as u64) << 32))
}

/// Java `__LO(x, low)`
#[inline(always)]
fn with_lo(x: f64, low: i32) -> f64 {
    f64::from_bits((x.to_bits() & 0xffff_ffff_0000_0000) | (low as u32 as u64))
}

/// Java `__HI_LO(high, low)`
#[inline(always)]
fn with_hi_lo(high: i32, low: i32) -> f64 {
    f64::from_bits(((high as u32 as u64) << 32) | (low as u32 as u64))
}

/// `Math.scalb(x, n)`：本移植只在参数归约内使用，n ∈ [-60, 30] 且 x 为规格化数/1.0，
/// 乘精确的 2^n 与 Java 语义一致（无次正规双重舍入）。
#[inline(always)]
fn scalb(x: f64, n: i32) -> f64 {
    debug_assert!((-600..=600).contains(&n));
    let p = if (-1022..=1023).contains(&n) {
        f64::from_bits(((1023 + n as i64) as u64) << 52)
    } else if n >= 0 {
        f64::INFINITY
    } else {
        0.0
    };
    x * p
}

/// fdlibm `__kernel_sin`
#[inline(always)]
fn kernel_sin(x: f64, y: f64, iy: i32) -> f64 {
    let ix = hi(x) & EXP_SIGNIF_BITS;
    if ix < 0x3e40_0000 {
        if x as i32 == 0 {
            return x;
        }
    }
    let z = x * x;
    let v = z * x;
    let r = SIN_S2 + z * (SIN_S3 + z * (SIN_S4 + z * (SIN_S5 + z * SIN_S6)));
    if iy == 0 {
        x + v * (SIN_S1 + z * r)
    } else {
        x - ((z * (0.5 * y - v * r) - y) - v * SIN_S1)
    }
}

/// fdlibm `__kernel_cos`
#[inline(always)]
fn kernel_cos(x: f64, y: f64) -> f64 {
    let ix = hi(x) & EXP_SIGNIF_BITS;
    if ix < 0x3e40_0000 {
        if x as i32 == 0 {
            return 1.0;
        }
    }
    let z = x * x;
    let r = z * (COS_C1 + z * (COS_C2 + z * (COS_C3 + z * (COS_C4 + z * (COS_C5 + z * COS_C6)))));
    if ix < 0x3fd3_3333 {
        1.0 - (0.5 * z - (z * r - x * y))
    } else {
        let qx = if ix > 0x3fe9_0000 {
            0.28125
        } else {
            with_hi_lo(ix - 0x0020_0000, 0)
        };
        let hz = 0.5 * z - qx;
        let a = 1.0 - qx;
        a - (hz - (z * r - x * y))
    }
}

/// fdlibm `__ieee754_rem_pio2`：返回 n（k mod 4 的原始 k），y[0]+y[1] = x - n*pi/2
fn rem_pio2(x: f64, y: &mut [f64; 2]) -> i32 {
    let hx = hi(x);
    let ix = hx & EXP_SIGNIF_BITS;
    if ix <= 0x3fe9_21fb {
        y[0] = x;
        y[1] = 0.0;
        return 0;
    }
    if ix < 0x4002_d97c {
        if hx > 0 {
            if ix != 0x3ff9_21fb {
                let z = x - REM_PIO2_PIO2_1;
                let y0 = z - REM_PIO2_PIO2_1T;
                y[0] = y0;
                y[1] = (z - y0) - REM_PIO2_PIO2_1T;
            } else {
                let z = x - REM_PIO2_PIO2_1 - REM_PIO2_PIO2_2;
                let y0 = z - REM_PIO2_PIO2_2T;
                y[0] = y0;
                y[1] = (z - y0) - REM_PIO2_PIO2_2T;
            }
            return 1;
        } else {
            if ix != 0x3ff9_21fb {
                let z = x + REM_PIO2_PIO2_1;
                let y0 = z + REM_PIO2_PIO2_1T;
                y[0] = y0;
                y[1] = (z - y0) + REM_PIO2_PIO2_1T;
            } else {
                let z = x + REM_PIO2_PIO2_1 + REM_PIO2_PIO2_2;
                let y0 = z + REM_PIO2_PIO2_2T;
                y[0] = y0;
                y[1] = (z - y0) + REM_PIO2_PIO2_2T;
            }
            return -1;
        }
    }
    if ix <= 0x4139_21fb {
        let t = x.abs();
        let n = (t * REM_PIO2_INVPIO2 + 0.5) as i32;
        let fn_ = n as f64;
        let mut r = t - fn_ * REM_PIO2_PIO2_1;
        let mut w = fn_ * REM_PIO2_PIO2_1T;
        if n < 32 && ix != REM_PIO2_NPIO2_HW[(n - 1) as usize] {
            y[0] = r - w;
        } else {
            let j = ix >> 20;
            y[0] = r - w;
            let mut i = j - ((hi(y[0]) >> 20) & 0x7ff);
            if i > 16 {
                let t2 = r;
                w = fn_ * REM_PIO2_PIO2_2;
                r = t2 - w;
                w = fn_ * REM_PIO2_PIO2_2T - ((t2 - r) - w);
                y[0] = r - w;
                i = j - ((hi(y[0]) >> 20) & 0x7ff);
                if i > 49 {
                    let t3 = r;
                    w = fn_ * REM_PIO2_PIO2_3;
                    r = t3 - w;
                    w = fn_ * REM_PIO2_PIO2_3T - ((t3 - r) - w);
                    y[0] = r - w;
                }
            }
        }
        y[1] = (r - y[0]) - w;
        return if hx < 0 {
            y[0] = -y[0];
            y[1] = -y[1];
            -n
        } else {
            n
        };
    }
    if ix >= EXP_BITS {
        let nan = x - x;
        y[0] = nan;
        y[1] = nan;
        return 0;
    }
    // 大参数路径：z = scalbn(|x|, ilogb(x)-23)
    let e0 = (ix >> 20) - 1046;
    let mut z = with_lo(0.0, lo(x));
    z = with_hi(z, ix - (e0 << 20));
    let mut tx = [0.0f64; 3];
    for slot in tx.iter_mut().take(2) {
        *slot = (z as i32) as f64;
        z = (z - *slot) * FDLIBM_TWO24;
    }
    tx[2] = z;
    let mut nx = 3;
    while tx[nx - 1] == 0.0 {
        nx -= 1;
    }
    let n = kernel_rem_pio2(&tx, y, e0, nx as i32, 2usize);
    if hx < 0 {
        y[0] = -y[0];
        y[1] = -y[1];
        -n
    } else {
        n
    }
}

/// fdlibm `__kernel_rem_pio2`（prec == 2 / double）
fn kernel_rem_pio2(x: &[f64; 3], y: &mut [f64; 2], e0: i32, nx: i32, prec: usize) -> i32 {
    let ipio2: &[i32; 66] = &REM_PIO2_TWO_OVER_PI;
    let mut iq = [0i32; 20];
    let mut f = [0.0f64; 20];
    let mut fq = [0.0f64; 20];
    let mut q = [0.0f64; 20];

    let jk = KERNEL_REM_PIO2_INIT_JK[prec];
    let jp = jk;
    let jx = nx - 1;
    let mut jv = (e0 - 3) / 24;
    if jv < 0 {
        jv = 0;
    }
    let mut q0 = e0 - 24 * (jv + 1);

    let mut j = jv - jx;
    let m = jx + jk;
    for i in 0..=m {
        f[i as usize] = if j < 0 { 0.0 } else { ipio2[j as usize] as f64 };
        j += 1;
    }
    for i in 0..=jk {
        let mut fw = 0.0;
        for j in 0..=jx {
            fw += x[j as usize] * f[(jx + i - j) as usize];
        }
        q[i as usize] = fw;
    }

    let mut jz = jk;
    let mut z;
    let mut n;
    let mut ih;
    loop {
        z = q[jz as usize];
        let mut i = 0;
        let mut j = jz;
        while j > 0 {
            let fw = ((KERNEL_REM_PIO2_TWON24 * z) as i32) as f64;
            iq[i] = (z - FDLIBM_TWO24 * fw) as i32;
            z = q[(j - 1) as usize] + fw;
            i += 1;
            j -= 1;
        }
        z = scalb(z, q0);
        z -= 8.0 * (z * 0.125).floor();
        n = z as i32;
        z -= n as f64;
        ih = 0;
        if q0 > 0 {
            let i2 = iq[(jz - 1) as usize] >> (24 - q0);
            n += i2;
            iq[(jz - 1) as usize] -= i2 << (24 - q0);
            ih = iq[(jz - 1) as usize] >> (23 - q0);
        } else if q0 == 0 {
            ih = iq[(jz - 1) as usize] >> 23;
        } else if z >= 0.5 {
            ih = 2;
        }
        if ih > 0 {
            n += 1;
            let mut carry = 0;
            for i3 in 0..jz {
                let jj = iq[i3 as usize];
                if carry == 0 {
                    if jj != 0 {
                        carry = 1;
                        iq[i3 as usize] = 0x100_0000 - jj;
                    }
                } else {
                    iq[i3 as usize] = 0xff_ffff - jj;
                }
            }
            if q0 > 0 {
                match q0 {
                    1 => iq[(jz - 1) as usize] &= 0x7f_ffff,
                    2 => iq[(jz - 1) as usize] &= 0x3f_ffff,
                    _ => {}
                }
            }
            if ih == 2 {
                z = 1.0 - z;
                if carry != 0 {
                    z -= scalb(1.0, q0);
                }
            }
        }
        if z == 0.0 {
            let mut jj = 0;
            let mut i4 = jz - 1;
            while i4 >= jk {
                jj |= iq[i4 as usize];
                i4 -= 1;
            }
            if jj == 0 {
                let mut k = 1;
                while iq[(jk - k) as usize] == 0 {
                    k += 1;
                }
                for i5 in (jz + 1)..=(jz + k) {
                    f[(jx + i5) as usize] = ipio2[(jv + i5) as usize] as f64;
                    let mut fw = 0.0;
                    for jj2 in 0..=jx {
                        fw += x[jj2 as usize] * f[(jx + i5 - jj2) as usize];
                    }
                    q[i5 as usize] = fw;
                }
                jz += k;
                continue;
            }
            break;
        }
        break;
    }

    if z == 0.0 {
        jz -= 1;
        q0 -= 24;
        while iq[jz as usize] == 0 {
            jz -= 1;
            q0 -= 24;
        }
    } else {
        z = scalb(z, -q0);
        if z >= FDLIBM_TWO24 {
            let fw = ((KERNEL_REM_PIO2_TWON24 * z) as i32) as f64;
            iq[jz as usize] = (z - FDLIBM_TWO24 * fw) as i32;
            jz += 1;
            q0 += 24;
            iq[jz as usize] = fw as i32;
        } else {
            iq[jz as usize] = z as i32;
        }
    }

    let mut fw = scalb(1.0, q0);
    let mut i = jz;
    while i >= 0 {
        q[i as usize] = fw * iq[i as usize] as f64;
        fw *= KERNEL_REM_PIO2_TWON24;
        i -= 1;
    }
    i = jz;
    while i >= 0 {
        let mut fw2 = 0.0;
        let mut k = 0;
        while k <= jp && k <= jz - i {
            fw2 += KERNEL_REM_PIO2_P_IO2[k as usize] * q[(i + k) as usize];
            k += 1;
        }
        fq[(jz - i) as usize] = fw2;
        i -= 1;
    }

    fw = 0.0;
    i = jz;
    while i >= 0 {
        fw += fq[i as usize];
        i -= 1;
    }
    y[0] = if ih == 0 { fw } else { -fw };
    fw = fq[0] - fw;
    i = 1;
    while i <= jz {
        fw += fq[i as usize];
        i += 1;
    }
    y[1] = if ih == 0 { fw } else { -fw };
    n & 7
}

/// Java `StrictMath.sin`
pub fn sin(x: f64) -> f64 {
    let ix = hi(x) & EXP_SIGNIF_BITS;
    if ix <= 0x3fe9_21fb {
        return kernel_sin(x, 0.0, 0);
    }
    if ix >= EXP_BITS {
        return x - x;
    }
    let mut y = [0.0f64; 2];
    let n = rem_pio2(x, &mut y);
    match n & 3 {
        0 => kernel_sin(y[0], y[1], 1),
        1 => kernel_cos(y[0], y[1]),
        2 => -kernel_sin(y[0], y[1], 1),
        _ => -kernel_cos(y[0], y[1]),
    }
}

/// Java `StrictMath.cos`
pub fn cos(x: f64) -> f64 {
    let ix = hi(x) & EXP_SIGNIF_BITS;
    if ix <= 0x3fe9_21fb {
        return kernel_cos(x, 0.0);
    }
    if ix >= EXP_BITS {
        return x - x;
    }
    let mut y = [0.0f64; 2];
    let n = rem_pio2(x, &mut y);
    match n & 3 {
        0 => kernel_cos(y[0], y[1]),
        1 => -kernel_sin(y[0], y[1], 1),
        2 => -kernel_cos(y[0], y[1]),
        _ => kernel_sin(y[0], y[1], 1),
    }
}

/// Java `StrictMath.atan`
pub fn atan(x: f64) -> f64 {
    let hx = hi(x);
    let ix = hx & EXP_SIGNIF_BITS;
    let mut x = x;
    let id;
    if ix >= 0x4410_0000 {
        if ix > EXP_BITS || (ix == EXP_BITS && lo(x) != 0) {
            return x + x;
        }
        return if hx > 0 {
            ATAN_ATANHI[3] + ATAN_ATANLO[3]
        } else {
            -ATAN_ATANHI[3] - ATAN_ATANLO[3]
        };
    }
    if ix < 0x3fdc_0000 {
        if ix < 0x3e20_0000 {
            if FDLIBM_HUGE + x > 1.0 {
                return x;
            }
        }
        id = -1;
    } else {
        x = x.abs();
        if ix < 0x3ff3_0000 {
            if ix < 0x3fe6_0000 {
                id = 0;
                x = (2.0 * x - 1.0) / (2.0 + x);
            } else {
                id = 1;
                x = (x - 1.0) / (x + 1.0);
            }
        } else if ix < 0x4003_8000 {
            id = 2;
            x = (x - 1.5) / (1.0 + 1.5 * x);
        } else {
            id = 3;
            x = -1.0 / x;
        }
    }
    let z = x * x;
    let w = z * z;
    let s1 = z
        * (ATAN_A_T[0]
            + w * (ATAN_A_T[2]
                + w * (ATAN_A_T[4] + w * (ATAN_A_T[6] + w * (ATAN_A_T[8] + w * ATAN_A_T[10])))));
    let s2 = w
        * (ATAN_A_T[1]
            + w * (ATAN_A_T[3] + w * (ATAN_A_T[5] + w * (ATAN_A_T[7] + w * ATAN_A_T[9]))));
    if id < 0 {
        x - x * (s1 + s2)
    } else {
        let zz = ATAN_ATANHI[id as usize] - ((x * (s1 + s2) - ATAN_ATANLO[id as usize]) - x);
        if hx < 0 { -zz } else { zz }
    }
}

/// Java `StrictMath.atan2`
pub fn atan2(y: f64, x: f64) -> f64 {
    let hx = hi(x);
    let ix = hx & EXP_SIGNIF_BITS;
    let lx = lo(x);
    let hy = hi(y);
    let iy = hy & EXP_SIGNIF_BITS;
    let ly = lo(y);
    if x.is_nan() || y.is_nan() {
        return x + y;
    }
    if (hx.wrapping_sub(0x3ff0_0000) | lx) == 0 {
        return atan(y);
    }
    let m = ((hy >> 31) & 1) | ((hx >> 30) & 2);
    if (iy | ly) == 0 {
        match m {
            2 => return MATH_PI + ATAN2_TINY,
            3 => return -MATH_PI - ATAN2_TINY,
            _ => return y,
        }
    }
    if (ix | lx) == 0 {
        return if hy < 0 {
            -ATAN2_PI_O_2 - ATAN2_TINY
        } else {
            ATAN2_PI_O_2 + ATAN2_TINY
        };
    }
    if ix == EXP_BITS {
        if iy == EXP_BITS {
            return match m {
                0 => ATAN2_PI_O_4 + ATAN2_TINY,
                1 => -ATAN2_PI_O_4 - ATAN2_TINY,
                2 => 3.0 * ATAN2_PI_O_4 + ATAN2_TINY,
                _ => -3.0 * ATAN2_PI_O_4 - ATAN2_TINY,
            };
        }
        return match m {
            0 => 0.0,
            1 => -0.0,
            2 => MATH_PI + ATAN2_TINY,
            _ => -MATH_PI - ATAN2_TINY,
        };
    }
    if iy == EXP_BITS {
        return if hy < 0 {
            -ATAN2_PI_O_2 - ATAN2_TINY
        } else {
            ATAN2_PI_O_2 + ATAN2_TINY
        };
    }
    let k = (iy - ix) >> 20;
    let z = if k > 60 {
        ATAN2_PI_O_2 + 0.5 * ATAN2_PI_LO
    } else if hx < 0 && k < -60 {
        0.0
    } else {
        atan((y / x).abs())
    };
    match m {
        0 => z,
        1 => -z,
        2 => MATH_PI - (z - ATAN2_PI_LO),
        _ => (z - ATAN2_PI_LO) - MATH_PI,
    }
}

/// Java `Math.toRadians`（`StrictMath` 委托同一实现：`angdeg * DEGREES_TO_RADIANS`）
#[inline(always)]
pub fn to_radians(angdeg: f64) -> f64 {
    angdeg * MATH_DEGREES_TO_RADIANS
}

/// Java `Math.toDegrees`（`StrictMath` 委托同一实现：`angrad * RADIANS_TO_DEGREES`）
#[inline(always)]
pub fn to_degrees(angrad: f64) -> f64 {
    angrad * MATH_RADIANS_TO_DEGREES
}
