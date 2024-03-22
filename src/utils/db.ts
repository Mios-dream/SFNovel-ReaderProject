
import { createClient } from '@supabase/supabase-js'

export const Server = createClient(process.env.SUPABASE_URL ?? "", process.env.SUPABASE_SERVICE_KEY ?? "")

export const Public = createClient(process.env.SUPABASE_URL ?? "", process.env.SUPABASE_ANON_KEY ?? "")

