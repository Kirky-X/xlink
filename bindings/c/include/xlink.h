#ifndef XPUSH_C_BINDINGS_H
#define XPUSH_C_BINDINGS_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// --- 基础类型定义 ---

typedef struct {
    uint8_t data[16];
} xlink_uuid_t;

typedef xlink_uuid_t xlink_device_id_t;
typedef xlink_uuid_t xlink_group_id_t;

typedef enum {
    XPUSH_PRIORITY_LOW = 0,
    XPUSH_PRIORITY_NORMAL = 1,
    XPUSH_PRIORITY_HIGH = 2,
    XPUSH_PRIORITY_CRITICAL = 3
} xlink_priority_t;

// --- SDK 生命周期管理 ---

typedef struct xlink_sdk xlink_sdk_t;

/**
 * 初始化 SDK
 * @return SDK 指针，失败返回 NULL
 */
xlink_sdk_t* xlink_init();

/**
 * 释放 SDK 资源
 * @param sdk SDK 指针
 */
void xlink_free(xlink_sdk_t* sdk);

// --- 消息操作 ---

/**
 * 发送文本消息给指定设备
 * @param sdk SDK 指针
 * @param target 目标设备 ID
 * @param text 文本内容
 * @return 0 表示成功，非 0 表示错误码
 */
int32_t xlink_send_text(xlink_sdk_t* sdk, xlink_device_id_t target, const char* text);

/**
 * 向群组广播消息
 * @param sdk SDK 指针
 * @param group_id 群组 ID
 * @param text 文本内容
 * @return 0 表示成功，非 0 表示错误码
 */
int32_t xlink_broadcast_text(xlink_sdk_t* sdk, xlink_group_id_t group_id, const char* text);

#ifdef __cplusplus
}
#endif

#endif // XPUSH_C_BINDINGS_H
