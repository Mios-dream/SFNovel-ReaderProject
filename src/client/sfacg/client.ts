import { Category, ChapterInfo, NovelInfo, Options, Tag, UserInfo, VolumeInfos, Client } from './stuct';
import { SFACG } from './http';
import fs from 'fs';
import path from 'path';


export class SfacgClient extends SFACG implements Client {
  
    addAconut(userName: string, passWord: string): void {
        const accountToAdd = {
            username: userName,
            password: passWord
        }
        const outputFloder = path.join( SFACG.APP_NAME, 'Config')
        const outputFile = path.join(outputFloder, 'Acoount.json')
        if (!fs.existsSync(outputFloder)) {
            fs.mkdirSync(outputFloder)
        }
        if (!fs.existsSync(outputFile)) {
            fs.writeFileSync(outputFile, JSON.stringify(accountToAdd))
        } else {
            let accountInfo = {}
        }
    }

    async login(username: string, password: string): Promise<Boolean> {
        try {
            let res: any = await this.post("/sessions", {
            userName: username,
            passWord: password
            });
            return (res.status == 200)?true:false
        } catch (err: any) {
            console.error(`${username} LOGIN failed : ${JSON.stringify(err.response.data.status.msg)}`)
            return false;
        }
    }

    async userInfo(): Promise<any> {
        try {
            let res: any = await this.get("/user")
            return res.data.data
        } catch (err: any) {
            console.error(`An error occured GET UserInfo : ${JSON.stringify(err.response.data.status.msg)}`)
        }
    }

    async novelInfo(id: number): Promise<NovelInfo | null> {
        return null
    }

    async volumeInfos(id: number): Promise<VolumeInfos> {
        
        return []
    }

    async contentInfos(info: ChapterInfo): Promise<any> {
        
    }

    async image(url: URL): Promise<any> {
      
    }

    async searchInfos<T>(text: T, page: number, size: number): Promise<number[]> {
        
        return []
    }


    async bookshelfInfos(): Promise<number[]> {
        
        return []
    }

    async categories(): Promise<Category[]> {
        
        return []
    }

    async tags(): Promise<Tag[]> {
       
        return []
    }

    async novels(option: Options, page: number, size: number): Promise<number[]> {
        
        return []
    }



}

