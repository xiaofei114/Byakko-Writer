//! 敏感信息（API Key）的本地存储保护
//!
//! - Windows 下使用 DPAPI（`CryptProtectData`），密钥由当前用户账户派生，
//!   离开这台机器/这个用户账户无法解密。
//! - 其它平台没有等价系统接口，退回明文存储（与旧版本行为一致）。
//!
//! 存储格式：`dpapi:v1:<hex>`。读取时兼容历史明文（无前缀）并按需迁移。

/// 加密后的前缀标识
const PREFIX: &str = "dpapi:v1:";

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    Some(out)
}

/// 加密敏感字符串；失败或非 Windows 平台返回原值
pub fn protect_secret(plain: &str) -> String {
    if plain.is_empty() {
        return String::new();
    }

    #[cfg(windows)]
    {
        if let Ok(bytes) = dpapi::protect(plain.as_bytes()) {
            return format!("{}{}", PREFIX, to_hex(&bytes));
        }
        log::warn!("[Secret] DPAPI 加密失败，本次将明文写入配置");
    }

    plain.to_string()
}

/// 解密敏感字符串；兼容历史明文
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn unprotect_secret(stored: &str) -> String {
    if stored.is_empty() {
        return String::new();
    }

    let Some(hex) = stored.strip_prefix(PREFIX) else {
        // 历史明文，原样返回
        return stored.to_string();
    };

    #[cfg(windows)]
    {
        if let Some(bytes) = from_hex(hex) {
            match dpapi::unprotect(&bytes) {
                Ok(plain) => return String::from_utf8_lossy(&plain).to_string(),
                Err(e) => {
                    log::warn!("[Secret] DPAPI 解密失败（可能更换了电脑或用户），请重新填写 API Key: {}", e);
                    return String::new();
                }
            }
        }
        log::warn!("[Secret] 加密数据格式损坏，请重新填写 API Key");
    }

    String::new()
}

/// 数据是否已加密
pub fn is_protected(stored: &str) -> bool {
    stored.starts_with(PREFIX)
}

#[cfg(windows)]
mod dpapi {
    use std::ffi::c_void;

    /// Windows 的 CRYPT_INTEGER_BLOB / DATA_BLOB
    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            p_data_in: *const DataBlob,
            sz_data_descr: *const u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *const c_void,
            p_prompt_struct: *const c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            p_data_in: *const DataBlob,
            ppsz_data_descr: *mut *mut u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *const c_void,
            p_prompt_struct: *const c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(h_mem: *mut c_void) -> *mut c_void;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    pub fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
        let input = DataBlob {
            cb_data: plain.len() as u32,
            pb_data: plain.as_ptr() as *mut u8,
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: std::ptr::null_mut(),
        };

        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };

        if ok == 0 || output.pb_data.is_null() {
            return Err("CryptProtectData 调用失败".to_string());
        }

        let bytes = unsafe {
            std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec()
        };
        unsafe {
            LocalFree(output.pb_data as *mut c_void);
        }

        Ok(bytes)
    }

    pub fn unprotect(cipher: &[u8]) -> Result<Vec<u8>, String> {
        let input = DataBlob {
            cb_data: cipher.len() as u32,
            pb_data: cipher.as_ptr() as *mut u8,
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: std::ptr::null_mut(),
        };

        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };

        if ok == 0 || output.pb_data.is_null() {
            return Err("CryptUnprotectData 调用失败".to_string());
        }

        let bytes = unsafe {
            std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec()
        };
        unsafe {
            LocalFree(output.pb_data as *mut c_void);
        }

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_is_passthrough() {
        assert_eq!(unprotect_secret("sk-plain"), "sk-plain");
        assert_eq!(unprotect_secret(""), "");
        assert!(!is_protected("sk-plain"));
    }

    #[test]
    fn hex_roundtrip() {
        let bytes = vec![0u8, 1, 15, 16, 255];
        assert_eq!(from_hex(&to_hex(&bytes)).unwrap(), bytes);
        assert!(from_hex("abc").is_none());
        assert!(from_hex("zz").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_roundtrip() {
        let secret = "sk-test-1234567890-中文密钥";
        let protected = protect_secret(secret);
        assert!(is_protected(&protected), "应返回 dpapi 前缀: {}", protected);
        assert_eq!(unprotect_secret(&protected), secret);
    }
}
