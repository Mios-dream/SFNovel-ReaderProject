
import fs from "fs-extra";
import path from "path"



export abstract class accountManager {
    account: any
    saveAccountPath: string
    constructor(appName: string) {
        this.saveAccountPath = path.resolve(__dirname, `../output/Accounts/${appName}.json`)
        if (!fs.pathExistsSync(this.saveAccountPath)) {
            fs.outputJSONSync(this.saveAccountPath, { data: [] })
        }
        const saveAccountInfo: any = fs.readJSONSync(this.saveAccountPath);
        this.account = saveAccountInfo
    }
}

