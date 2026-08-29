import readline from "readline"
import { exec } from 'child_process';

/**
 * 调用 Pandoc 将 Markdown 文件转换为 EPUB。
 * @param outputDir Pandoc 执行时使用的工作目录。
 * @param mdFilePath 输入 Markdown 文件路径。
 * @param epubFilePath 输出 EPUB 文件路径。
 * @returns 转换成功或失败的 Promise。
 */
export function epubMaker(outputDir: string, mdFilePath: string, epubFilePath: string): Promise<void> {
    return new Promise((resolve, reject) => {
        const pandocCommand = `cd ${outputDir}&&pandoc  ${mdFilePath}  -o ${epubFilePath} --from=commonmark+yaml_metadata_block --to=epub3 --split-level=2 --epub-title-page=false `;
        exec(pandocCommand, (error, stdout, stderr) => {
            if (error) {
                console.error(`执行的错误: ${error}`);
                reject(error);
                return;
            }
            console.log(`stdout: ${stdout}`);
            console.error(`stderr: ${stderr}`);
            resolve();
        });
    });
}
/**
 * 在命令行显示问题并等待用户输入。
 * @param query 要显示的提示文本。
 * @returns 用户输入内容的 Promise。
 */
export function question(query: any) {
    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });
    return new Promise((resolve) => {
        rl.question(query, (answer) => {
            rl.close();
            resolve(answer);
        });
    });
}

/**
 * 依次询问并返回账号和密码。
 * @returns 包含 userName 和 passWord 的账号对象。
 */
export async function questionAccount() {
    const userName = await question(colorize("输入账号："
        , "purple"));
    const passWord = await question(colorize("输入密码："
        , "purple"));
    return { userName, passWord }
}


// 返回带颜色的字
/**
 * 为终端文本添加 ANSI 颜色控制码。
 * @param text 待着色文本。
 * @param color 支持的颜色名称。
 * @returns 包含颜色控制码的文本。
 */
export function colorize(text: string, color: "blue" | "green" | "purple" | "yellow") {
    const colors = {
        blue: "\x1b[34m",
        green: "\x1b[32m",
        purple: "\x1b[35m",
        yellow: "\x1b[33m"
    };
    return colors[color] + text + "\x1b[0m";
}

// 生成随机昵称
/**
 * 生成两个汉字加四位字母数字的随机昵称。
 * @returns 随机昵称字符串。
 */
export function RandomName() {
    // 生成随机六位汉字+字母+数字组合的代码
    const random = (min: number, max: number) => Math.floor(Math.random() * (max - min) + min);
    const randomChar = (length: number) => {
        const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
        let result = '';
        for (let i = 0; i < length; i++) {
            result += chars[random(0, chars.length)];
        }
        return result;
    };
    const randomChinese = (length: number) => {
        let result = '';
        for (let i = 0; i < length; i++) {
            result += String.fromCharCode(random(0x4e00, 0x9fa5));
        }
        return result;
    };
    const randomName = randomChinese(2) + randomChar(4);
    return randomName;
}

// 隐藏手机号
/**
 * 隐藏手机号中间四位数字。
 * @param phoneNumber 原始手机号。
 * @returns 脱敏后的手机号。
 */
export function Secret(phoneNumber: string) {
    return phoneNumber.replace(/(\d{3})\d{4}(\d{4})/, '$1****$2');
}

// 获得格式化时间xxxx-xx-xx
/**
 * 获取当前东八区日期。
 * @returns `YYYY-MM-DD` 格式的日期字符串。
 */
export function getNowFormatDate(): string {
    const date = new Date();
    const utc8Offset = 8 * 60;
    const now = new Date(date.getTime() + utc8Offset * 60 * 1000);
    const year = now.getUTCFullYear();
    const month = (now.getUTCMonth() + 1).toString().padStart(2, "0");
    const day = now.getUTCDate().toString().padStart(2, "0");
    return `${year}-${month}-${day}`;
}
