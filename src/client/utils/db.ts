
import { createClient } from '@supabase/supabase-js'
import dotenv from 'dotenv';
dotenv.config();

// 分别提供服务端密钥客户端和公开匿名客户端，凭据通过环境变量注入。
export const Server = createClient(process.env.SUPABASE_URL ?? "", process.env.SUPABASE_SERVICE_KEY ?? "")

export const Public = createClient(process.env.SUPABASE_URL ?? "", process.env.SUPABASE_ANON_KEY ?? "")

