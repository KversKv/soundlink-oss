// opus_jni.c — libopus 的 JNI 桥接。
//
// 实现 com.soundlink.soundlink.codec.OpusEncoder 的 native 方法。
// 依赖：libopus（CMake 由 opus 源码或预编译库构建）。

#include <jni.h>
#include <stdlib.h>
#include <string.h>
#include <opus/opus.h>

#define ENC_APPLICATION OPUS_APPLICATION_AUDIO

JNIEXPORT jlong JNICALL
Java_com_soundlink_soundlink_codec_OpusEncoder_nativeCreate(
    JNIEnv *env, jclass clazz, jint sampleRate, jint channels, jint bitrate) {
    (void)env; (void)clazz;
    int err = 0;
    OpusEncoder *enc = opus_encoder_create(sampleRate, channels, ENC_APPLICATION, &err);
    if (err != OPUS_OK || enc == NULL) {
        return 0;
    }
    opus_encoder_ctl(enc, OPUS_SET_BITRATE(bitrate));
    opus_encoder_ctl(enc, OPUS_SET_COMPLEXITY(10));
    opus_encoder_ctl(enc, OPUS_SET_SIGNAL(OPUS_SIGNAL_MUSIC));
    opus_encoder_ctl(enc, OPUS_SET_PACKET_LOSS_PERC(0));
    return (jlong)(intptr_t)enc;
}

JNIEXPORT jbyteArray JNICALL
Java_com_soundlink_soundlink_codec_OpusEncoder_nativeEncode(
    JNIEnv *env, jclass clazz, jlong ptr, jshortArray pcmArr, jint frameSize) {
    (void)clazz;
    OpusEncoder *enc = (OpusEncoder *)(intptr_t)ptr;
    if (enc == NULL) {
        return NULL;
    }
    jshort *pcm = (*env)->GetShortArrayElements(env, pcmArr, NULL);
    if (pcm == NULL) {
        return NULL;
    }
    unsigned char buf[1276];
    int n = opus_encode(enc, (const opus_int16 *)pcm, frameSize, buf, sizeof(buf));
    (*env)->ReleaseShortArrayElements(env, pcmArr, pcm, JNI_ABORT);
    if (n <= 0) {
        return NULL;
    }
    jbyteArray out = (*env)->NewByteArray(env, n);
    (*env)->SetByteArrayRegion(env, out, 0, n, (const jbyte *)buf);
    return out;
}

JNIEXPORT void JNICALL
Java_com_soundlink_soundlink_codec_OpusEncoder_nativeDestroy(
    JNIEnv *env, jclass clazz, jlong ptr) {
    (void)env; (void)clazz;
    OpusEncoder *enc = (OpusEncoder *)(intptr_t)ptr;
    if (enc != NULL) {
        opus_encoder_destroy(enc);
    }
}
