#import "opus/opus.h"
#import "opus/opus_defines.h"
#import "opus/opus_types.h"
#import "opus/opus_multistream.h"
#import "opus/opus_projection.h"
#import "opus/opus_custom.h"

// Swift 不能直接调用可变参数的 opus_encoder_ctl / opus_decoder_ctl，
// 在此提供 static inline 包装，供 Swift 端使用。
static inline int opus_encoder_set_bitrate(OpusEncoder *st, opus_int32 value) {
    return opus_encoder_ctl(st, OPUS_SET_BITRATE_REQUEST, value);
}
static inline int opus_encoder_set_complexity(OpusEncoder *st, opus_int32 value) {
    return opus_encoder_ctl(st, OPUS_SET_COMPLEXITY_REQUEST, value);
}
static inline int opus_encoder_set_signal(OpusEncoder *st, opus_int32 value) {
    return opus_encoder_ctl(st, OPUS_SET_SIGNAL_REQUEST, value);
}
static inline int opus_encoder_set_packet_loss_perc(OpusEncoder *st, opus_int32 value) {
    return opus_encoder_ctl(st, OPUS_SET_PACKET_LOSS_PERC_REQUEST, value);
}
