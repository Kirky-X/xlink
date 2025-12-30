use crate::core::traits::MessageHandler;
use crate::core::types::{ChannelType, DeviceCapabilities, DeviceId, DeviceType, MessagePayload};
use crate::UnifiedPushSDK;
use std::collections::HashSet;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Arc;
use tokio::runtime::Runtime;
use uuid::Uuid;

#[repr(C)]
pub struct xpush_sdk {
    pub(crate) inner: Arc<UnifiedPushSDK>,
    pub(crate) rt: Runtime,
}

struct NoopHandler;
#[async_trait::async_trait]
impl MessageHandler for NoopHandler {
    async fn handle_message(
        &self,
        _message: crate::core::types::Message,
    ) -> crate::core::error::Result<()> {
        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn xpush_init() -> *mut xpush_sdk {
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return std::ptr::null_mut(),
    };

    let sdk_future = async {
        let mut supported_channels = HashSet::new();
        supported_channels.insert(ChannelType::BluetoothLE);
        supported_channels.insert(ChannelType::Lan);

        let device_id = DeviceId::new();
        let config = DeviceCapabilities {
            device_id,
            device_type: DeviceType::Smartphone,
            device_name: "C-Binding-Device".to_string(),
            supported_channels,
            battery_level: Some(100),
            is_charging: true,
            data_cost_sensitive: false,
        };

        let handler = Arc::new(NoopHandler);
        let memory_channel = Arc::new(crate::channels::memory::MemoryChannel::new(handler, 10));
        let channels: Vec<Arc<dyn crate::core::traits::Channel>> = vec![memory_channel];

        UnifiedPushSDK::new(config, channels).await
    };

    match rt.block_on(sdk_future) {
        Ok(sdk) => {
            let boxed = Box::new(xpush_sdk {
                inner: Arc::new(sdk),
                rt,
            });
            Box::into_raw(boxed)
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// 关闭 SDK
///
/// # Safety
///
/// - `sdk` 必须是一个有效的、由 `xpush_init` 返回的指针
/// - 调用此函数后，`sdk` 指针将失效，不应再使用
#[no_mangle]
pub unsafe extern "C" fn xpush_shutdown(sdk: *mut xpush_sdk) -> i32 {
    if sdk.is_null() {
        return -1;
    }

    let sdk_ref = &*sdk;
    let shutdown_result: Result<(), ()> = sdk_ref.rt.block_on(async {
        sdk_ref.inner.stop().await;
        Ok(())
    });

    match shutdown_result {
        Ok(_) => 0,
        Err(_) => -2,
    }
}

/// 释放 SDK 资源
///
/// # Safety
///
/// - `sdk` 必须是一个有效的、由 `xpush_init` 返回的指针
/// - 调用此函数后，`sdk` 指针将失效，不应再使用
/// - 此函数会释放内存，因此不能对同一个指针调用两次
#[no_mangle]
pub unsafe extern "C" fn xpush_free(sdk: *mut xpush_sdk) {
    if sdk.is_null() {
        return;
    }

    let sdk_ref = &*sdk;

    sdk_ref.rt.block_on(async {
        sdk_ref.inner.stop().await;
    });

    let boxed = Box::from_raw(sdk);
    drop(boxed);
}

/// 发送文本消息
///
/// # Safety
///
/// 该函数必须由 C 调用，且参数必须有效：
/// - `sdk` 必须是一个有效的、由 `xpush_init` 返回的指针
/// - `target_ptr` 必须指向有效的设备ID数据
/// - `text` 必须指向有效的 UTF-8 字符串
#[no_mangle]
pub unsafe extern "C" fn xpush_send_text(
    sdk: *mut xpush_sdk,
    target_ptr: *const u8,
    text: *const c_char,
) -> i32 {
    if sdk.is_null() {
        return -1;
    }

    if target_ptr.is_null() {
        return -4;
    }

    if text.is_null() {
        return -5;
    }

    let sdk_ref = &*sdk;
    let c_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    let target_bytes: [u8; 16] = std::slice::from_raw_parts(target_ptr, 16)
        .try_into()
        .unwrap();

    let target_uuid = Uuid::from_slice(&target_bytes);
    if target_uuid.is_err() {
        return -7;
    }

    let target_id = DeviceId(target_uuid.unwrap());
    let payload = MessagePayload::Text(c_str.to_string());

    match sdk_ref.rt.block_on(sdk_ref.inner.send(target_id, payload)) {
        Ok(_) => 0,
        Err(_) => -3,
    }
}

/// 广播文本消息到群组
///
/// # Safety
///
/// - `sdk` 必须是有效的。
/// - `group_id_ptr` 必须指向 16 字节的 UUID。
/// - `text` 必须是有效的以 null 结尾的 C 字符串。
#[no_mangle]
pub unsafe extern "C" fn xpush_broadcast_text(
    sdk: *mut xpush_sdk,
    group_id_ptr: *const u8,
    text: *const c_char,
) -> i32 {
    if sdk.is_null() {
        return -1;
    }

    if group_id_ptr.is_null() {
        return -4;
    }

    if text.is_null() {
        return -5;
    }

    let sdk_ref = &*sdk;
    let c_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    let group_bytes: [u8; 16] = std::slice::from_raw_parts(group_id_ptr, 16)
        .try_into()
        .unwrap();

    let group_uuid = Uuid::from_slice(&group_bytes);
    if group_uuid.is_err() {
        return -7;
    }

    let group_id = crate::core::types::GroupId(group_uuid.unwrap());
    let payload = MessagePayload::Text(c_str.to_string());

    match sdk_ref
        .rt
        .block_on(sdk_ref.inner.send_to_group(group_id, payload))
    {
        Ok(_) => 0,
        Err(_) => -3,
    }
}
