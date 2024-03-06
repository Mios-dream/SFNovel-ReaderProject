import { URL } from 'url';


// You would need actual implementation for an Image class, this is just a placeholder

export interface AccountInfo {
    username: string
    password: string
}

export class UserInfo {
    nickname: string;
    constructor(nickname: string) {
        this.nickname = nickname;
    }
}

export class NovelInfo {
    id: number;
    name: string;
    authorName: string;
    coverUrl?: URL;
    introduction?: string[];
    wordCount?: number;
    isFinished?: boolean;
    createTime?: Date;
    updateTime?: Date;
    category?: Category;
    tags?: Tag[];

    constructor(id: number, name: string, authorName: string) {
        this.id = id;
        this.name = name;
        this.authorName = authorName;
    }

    equals(other: NovelInfo): boolean {
        return this.id === other.id;
    }
}

export class Category {
    id?: number;
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    toString(): string {
        return this.name;
    }
}

export class Tag {
    id?: number;
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    toString(): string {
        return this.name;
    }
}

export type VolumeInfos = VolumeInfo[];

export class VolumeInfo {
    title: string;
    chapterInfos: ChapterInfo[];

    constructor(title: string, chapterInfos: ChapterInfo[]) {
        this.title = title;
        this.chapterInfos = chapterInfos;
    }
}

export class ChapterInfo {
    identifier: unknown;
    title: string;
    isVip?: boolean;
    isAccessible?: boolean;
    isValid?: boolean;
    wordCount?: number;
    updateTime?: Date;

    constructor(identifier: unknown, title: string) {
        this.identifier = identifier;
        this.title = title;
    }

    _isAccessible(): boolean {
        return this.isAccessible !== false;
    }

    _isValid(): boolean {
        return this.isValid !== false;
    }

    canDownload(): boolean {
        return this._isAccessible() && this._isValid();
    }
}



export class Id {
    value: number;

    constructor(value: number) {
        this.value = value;
    }

    toString(): string {
        return this.value.toString();
    }
}



export type ContentInfos = ContentInfo[];

export class ContentInfo {

    kind: 'text' | 'image';
    content: string | URL;

    constructor(kind: 'text' | 'image', content: string | URL) {
        this.kind = kind;
        this.content = content;
    }
}

export class Options {
    isFinished?: boolean;
    isVip?: boolean;
    category?: Category;
    tags?: Tag[];
    excludedTags?: Tag[];
    updateDays?: number;
    wordCount?: unknown;
}

// Placeholder imports for `Url`, `DynamicImage`, `Error`, and other models you will use in implementation
// You should replace these with your actual imports or definitions based on your codebase

// Error handling in TypeScript is typically done using try/catch with the throwing of Error objects
class Error { }
export interface Client {
    // Add cookie
    addAconut(userName: string, passWord: string, outPutPATH: URL): void;

    // Quary account
    queryAcconut(userName: string, passWord: string): Promise<boolean>

    // Login
    login(username: string, password: string): Promise<void>;

    // Get the information of the logged-in user
    userInfo(): Promise<UserInfo | null>;

    // Get Novel Information
    novelInfo(id: number): Promise<NovelInfo | null>;

    // Get volume Information
    volumeInfos(id: number): Promise<VolumeInfos>;

    // Get content Information
    contentInfos(info: ChapterInfo): Promise<ContentInfos>;

    // Download image
    image(url: URL): Promise<unknown>;

    // Search, return novel id
    searchInfos(text: string, page: number, size: number): Promise<number[]>;

    // Get the favorite novel of the logged-in user and return the novel id
    bookshelfInfos(): Promise<number[]>;

    // Get all categories
    categories(): Promise<Category[]>;

    // Get all tags
    tags(): Promise<Tag[]>;

    // Search all matching novels
    novels(option: Options, page: number, size: number): Promise<number[]>;
}

// Define the rest of the types and classes/interfaces here as in previous messages

