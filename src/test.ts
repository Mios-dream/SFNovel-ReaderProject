import { SfacgClient } from "./client/sfacg/client";
import fs from 'fs';
import path from "path"

const client = new SfacgClient();
client.login("715494637", "dddd1111").then(userSession => {
    fs.writeFileSync("sfacg.json",userSession)
});