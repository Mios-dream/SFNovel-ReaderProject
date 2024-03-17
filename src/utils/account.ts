import { saveAccountInfo } from "../client/Sfacg/types/ITypes";
import fs from "fs-extra";
import path from "path"


export class accountManager {
    account: any
    constructor(appName: string) {
        const saveAccountPath = path.resolve(__dirname, `../output/Accounts/${appName}.json`)
        if (!fs.pathExistsSync(saveAccountPath)) {
            fs.outputJSONSync(saveAccountPath, { data: [] })
        }
        const saveAccountInfo: any = fs.readJSONSync(saveAccountPath);
        this.account = new Proxy(saveAccountInfo,
            {
                set(target: any, p: keyof any, newValue: any, receiver) {
                    target[p] = newValue;
                    fs.writeJSONSync(saveAccountPath, target, { spaces: 2 })
          
                    return true
                },
            }
        )
    }

}

