import crypto from "crypto"

export class ParseKsy {
    private static readonly SEED_16_CHARACTER: string = "zG2nSeEfSHfvTCHy5LCcqtBbQehKNLXn";
    private key: crypto.CipherKey;
    private iv: crypto.BinaryLike;

    constructor() {
        const sha256 = crypto.createHash('sha256');
        sha256.update(ParseKsy.SEED_16_CHARACTER);
        const keyBytes = sha256.digest()
        this.key = keyBytes;
        this.iv = Buffer.alloc(16, 0);
    }

    encrypt(str: string): string {
        const cipher = crypto.createCipheriv('aes-256-cbc', this.key, this.iv);
        let encrypted = cipher.update(str, 'utf8', 'base64');
        encrypted += cipher.final('base64');
        return encrypted;
    }

    decrypt(str: string): string {
        const decipher = crypto.createDecipheriv('aes-256-cbc', this.key, this.iv);
        const decrypted = Buffer.concat([
            decipher.update(Buffer.from(str, 'base64')),
            decipher.final(),
        ]);
        return decrypted.toString();
    }
}

