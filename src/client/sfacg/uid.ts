// import { SfacgClient } from "./client/sfacg/client";

// const client = new SfacgClient();
// client.login("715494637", "dddd1111").then(userSession => {
//     console.log(userSession)
// });

import { v4 as uuidv4 } from "uuid";
import { machineIdSync } from "node-machine-id";

let uid: string;
export function getUid(): string {
    if (!uid) {
        uid = machineIdSync() || uuidv4();
        uid = uid.toUpperCase();
    }
    return uid;
}
