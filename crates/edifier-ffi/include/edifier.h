#ifndef EDIFIER_H
#define EDIFIER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * EdifierCtrl C ABI.
 * 字符串约定: 返回的 char* 由 edifier_string_free 释放.
 * 失败返回 NULL, 用 edifier_last_error 取 UTF-8 原因.
 * JSON 字段名见 docs/ffi.md.
 */

const char *edifier_version(void);
const char *edifier_last_error(void);
void edifier_string_free(char *s);

/* 进程级原生日志, 在创建会话前注册. 首次成功注册的回调保留至进程退出.
 * 回调支持并发调用, 不抛异常, 不卸载其代码. 字符串仅在回调期间有效.
 * level: 1=error, 2=warn, 3=info, 4=debug, 5=trace. flush!=0 时须同步刷盘.
 * 两个函数成功返回 0, 失败返回 -1 并设置 edifier_last_error.
 */
typedef void (*EdifierLogCallback)(int level, const char *target, const char *message, int flush);
int edifier_log_install(EdifierLogCallback callback);
int edifier_log_set_level(int level);

char *edifier_command_encode(const char *command_json);
char *edifier_frame_parse(const char *frame_hex);
char *edifier_profiles_json(void);
char *edifier_readout_plan(const char *profile_key);
char *edifier_settings_parse(const char *settings_json);

char *edifier_group_id(const char *passphrase);
char *edifier_envelope_seal(const char *passphrase, uint64_t ts_ms, const char *message_json);
char *edifier_envelope_open(const char *passphrase, uint64_t now_ms, const char *envelope_json);

typedef struct EdifierDecoder EdifierDecoder;
EdifierDecoder *edifier_decoder_new(void);
void edifier_decoder_free(EdifierDecoder *decoder);
char *edifier_decoder_push_hex(EdifierDecoder *decoder, const char *hex);

typedef struct EdifierHandoff EdifierHandoff;
EdifierHandoff *edifier_handoff_new(const char *local_id);
void edifier_handoff_free(EdifierHandoff *machine);
void edifier_handoff_set_has_audio(EdifierHandoff *machine, int has_audio);
void edifier_handoff_set_can_control(EdifierHandoff *machine, int can_control);
char *edifier_handoff_claim(EdifierHandoff *machine, const char *mac, const char *nonce_hex, uint64_t now_ms);
char *edifier_handoff_on_message(EdifierHandoff *machine, const char *message_json, uint64_t now_ms);
char *edifier_handoff_tick(EdifierHandoff *machine, uint64_t now_ms);
char *edifier_handoff_on_audio_connected(EdifierHandoff *machine);
char *edifier_handoff_on_audio_disconnected(EdifierHandoff *machine);
char *edifier_handoff_on_audio_failed(EdifierHandoff *machine, const char *reason);

typedef struct EdifierSession EdifierSession;
EdifierSession *edifier_session_new(const char *local_id);
/* 同一 session 的调用由宿主串行执行. 阻塞调用和销毁不得占用 macOS main runloop. */
void edifier_session_free(EdifierSession *session);
void edifier_session_destroy(EdifierSession *session);
char *edifier_session_scan(EdifierSession *session, const char *kind);
int edifier_session_connect(EdifierSession *session, const char *address, const char *kind);
int edifier_session_disconnect(EdifierSession *session);
int edifier_session_readout(EdifierSession *session, const char *profile_key);
int edifier_session_send_json(EdifierSession *session, const char *command_json);
char *edifier_session_poll_event(EdifierSession *session);
int edifier_session_group_join(EdifierSession *session, const char *passphrase);
/* 幂等退出组并清理交接, 保留系统音频连接. */
int edifier_session_group_leave(EdifierSession *session);
char *edifier_session_group_peers(EdifierSession *session);
int edifier_session_group_claim(EdifierSession *session, const char *mac);
int edifier_session_group_claim_peer(EdifierSession *session, const char *peer_id);
char *edifier_session_group_id_hex(EdifierSession *session);
int edifier_session_set_holding(EdifierSession *session, const char *mac);
char *edifier_session_holding(EdifierSession *session);

#ifdef __cplusplus
}
#endif

#endif
