import { Category, ChapterInfo, NovelInfo, Options, Tag, UserInfo, VolumeInfos, AccountInfo, Client } from '../common/client';
import { URL } from 'url';
import { SFACG } from './utils';
import fs from 'fs';
import path from 'path';


export class SfacgClient extends SFACG implements Client {
  
    addAconut(userName: string, passWord: string): void {
        const accountInfo: AccountInfo = {
            username: userName,
            password: passWord
        }
        
        const outputFloder = path.join( SFACG.APP_NAME, 'Config')
        const outputFile = path.join(outputFloder, 'Acoount.json')
        if (!fs.existsSync(outputFloder)) {
            fs.mkdirSync(outputFloder)
        }
        if (!fs.existsSync(outputFile)) {
            fs.writeFileSync(outputFile, JSON.stringify(accountInfo))
        }
        fs.realpathSync
    }

    async queryAcconut(serName: string, passWord: string): Promise<boolean> {
        return true
    }

    async login(username: string, password: string): Promise<any> {
        
        const userSession = await this.post("/sessions", {
            user_name: username,
            pass_word: password
        });
        return userSession;
    }
    
    async userInfo(): Promise<UserInfo | null> {
     
        return null
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

