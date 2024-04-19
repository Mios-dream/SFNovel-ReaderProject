//
// Decompiled by Jadx (from NP Manager)
//
package com.kuangxiangciweimao.novel.utils;

import android.util.Base64;
import java.io.UnsupportedEncodingException;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.spec.AlgorithmParameterSpec;
import javax.crypto.Cipher;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.SecretKeySpec;

public class ParseKsy {
    public static final String SEED_16_CHARACTER = "zG2nSeEfSHfvTCHy5LCcqtBbQehKNLXn";
    private final Cipher cipher = MessageDigest.getInstance("AES/CBC/PKCS7Padding");
    private final SecretKeySpec key;
    private AlgorithmParameterSpec spec;

    public static String getMd5Value(String str) {
        try {
            MessageDigest instance = MessageDigest.getInstance("MD5");
            instance.update(str.getBytes("utf-8"));
            StringBuffer stringBuffer = new StringBuffer();
            byte[] digest = instance.digest();
            for (int i : digest) {
                int i2;
                if (i2 < 0) {
                    i2 += 256;
                }
                if (i2 < 16) {
                    stringBuffer.append("0");
                }
                stringBuffer.append(Integer.toHexString(i2));
            }
            return stringBuffer.toString();
        } catch (Exception e) {
            e.printStackTrace();
            return "";
        }
    }

    public static String md5Code(String str) {
        try {
            byte[] digest = MessageDigest.getInstance("MD5").digest(str.getBytes());
            StringBuffer stringBuffer = new StringBuffer();
            for (byte b : digest) {
                String toHexString = Integer.toHexString(b & 255);
                if (toHexString.length() < 2) {
                    StringBuilder stringBuilder = new StringBuilder();
                    stringBuilder.append("0");
                    stringBuilder.append(toHexString);
                    toHexString = stringBuilder.toString();
                }
                stringBuffer.append(toHexString);
            }
            return stringBuffer.toString();
        } catch (NoSuchAlgorithmException e) {
            e.printStackTrace();
            return "";
        }
    }

    public static String bytesToHexString(byte[] bArr) {
        StringBuilder stringBuilder = new StringBuilder("");
        if (bArr != null) {
            if (bArr.length > 0) {
                for (byte b : bArr) {
                    String toHexString = Integer.toHexString(b & 255);
                    if (toHexString.length() < 2) {
                        stringBuilder.append(0);
                    }
                    stringBuilder.append(toHexString);
                }
                return stringBuilder.toString();
            }
        }
        return null;
    }

    public static String stringToMD5(String str) {
        try {
            byte[] digest = MessageDigest.getInstance("MD5").digest(str.getBytes("UTF-8"));
            StringBuilder stringBuilder = new StringBuilder(digest.length * 2);
            for (byte b : digest) {
                int i = b & 255;
                if (i < 16) {
                    stringBuilder.append("0");
                }
                stringBuilder.append(Integer.toHexString(i));
            }
            return stringBuilder.toString();
        } catch (NoSuchAlgorithmException e) {
            e.printStackTrace();
            return null;
        } catch (UnsupportedEncodingException e2) {
            e2.printStackTrace();
            return null;
        }
    }

    public ParseKsy() throws Exception {
        MessageDigest instance = MessageDigest.getInstance("SHA-256");
        instance.update(SEED_16_CHARACTER.getBytes("UTF-8"));
        byte[] bArr = new byte[32];
        System.arraycopy(instance.digest(), 0, bArr, 0, 32);
        this.key = new SecretKeySpec(bArr, "AES");
        this.spec = getIV();
    }

    public static String getMD5111(String str) {
        try {
            MessageDigest instance = MessageDigest.getInstance("MD5");
            instance.update(str.getBytes("UTF-8"));
            return bytesToHexString(instance.digest());
        } catch (Exception e) {
            e.printStackTrace();
            return "";
        }
    }

    public String decrypt(String str) throws Exception {
        this.cipher.init(2, this.key, this.spec);
        String str2 = "UTF-8";
        return new String(this.cipher.doFinal(Base64.decode(str.getBytes(str2), 0)), str2);
    }

    public String encrypt(String str) throws Exception {
        this.cipher.init(1, this.key, this.spec);
        String str2 = "UTF-8";
        return new String(Base64.encode(this.cipher.doFinal(str.getBytes(str2)), 0), str2);
    }

    public AlgorithmParameterSpec getIV() {
        return new IvParameterSpec(new byte[]{(byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0, (byte) 0});
    }
}

  protected void doHttpRequest(Object... objArr) {
        HashMap hashMap = new HashMap();
        if (TextUtils.isEmpty(this.type)) {
            hashMap.put("phone_num", objArr[0].toString());
        } else {
            hashMap.put(this.type, objArr[0].toString());
        }
        hashMap.put("verify_type", objArr[1].toString());
        if (objArr.length > 2) {
            hashMap.put("login_name", objArr[2].toString());
        }
        long time = new Date().getTime();
        StringBuilder stringBuilder = new StringBuilder();
        stringBuilder.append(time);
        stringBuilder.append("");
        hashMap.put("timestamp", stringBuilder.toString());
        try {
            String account = LoginedUser.getLoginedUser().getReaderInfo().getAccount();
            StringBuilder stringBuilder2 = new StringBuilder();
            stringBuilder2.append(account);
            stringBuilder2.append(time);
            String str = "hashvalue";
            hashMap.put(str, ParseKsy.getMD5111(new ParseKsy().encrypt(stringBuilder2.toString()).trim()));
        } catch (Exception e) {
            e.printStackTrace(); 
        }
        getC(78, hashMap);
    }