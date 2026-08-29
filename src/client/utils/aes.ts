import crypto from "crypto"

/** 兼容旧版接口的 AES-256-CBC 加解密工具。 */
export class ParseKsy {
    private static readonly SEED_16_CHARACTER: string = "zG2nSeEfSHfvTCHy5LCcqtBbQehKNLXn";
    private key: crypto.CipherKey;
    private iv: crypto.BinaryLike;

    /** 使用固定种子派生 AES 密钥并初始化 CBC 向量。 */
    constructor() {
        const sha256 = crypto.createHash('sha256');
        sha256.update(ParseKsy.SEED_16_CHARACTER);
        const keyBytes = sha256.digest()
        this.key = keyBytes;
        this.iv = Buffer.alloc(16, 0);
    }

    /**
     * 使用 AES-256-CBC 加密文本。
     * @param str 待加密的 UTF-8 文本。
     * @returns Base64 编码的密文。
     */
    encrypt(str: string): string {
        const cipher = crypto.createCipheriv('aes-256-cbc', this.key, this.iv);
        let encrypted = cipher.update(str, 'utf8', 'base64');
        encrypted += cipher.final('base64');
        return encrypted;
    }

    /**
     * 使用 AES-256-CBC 解密 Base64 密文。
     * @param str Base64 编码的密文。
     * @returns 解密后的 UTF-8 文本。
     */
    decrypt(str: string): string {
        const decipher = crypto.createDecipheriv('aes-256-cbc', this.key, this.iv);
        const decrypted = Buffer.concat([
            decipher.update(Buffer.from(str, 'base64')),
            decipher.final(),
        ]);
        return decrypted.toString();
    }
}

const p = new ParseKsy()
const str = p.decrypt("IT+LcNazRBcK54/p1lMtc0mmJfVXWov795i6hcYtKO5ErwCHAPA0q9JARQ3vINB4lLjMGr+i3MaO04n9wBzP6f/sHi88t4orAnnfMj966H2ZFp4nPFZjDkMy4qgsGNXkFu6qOaj4VnCOFOIykp1iig==")
console.log(str);

