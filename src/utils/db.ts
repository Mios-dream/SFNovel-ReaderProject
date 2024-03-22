
import { createClient } from '@supabase/supabase-js'

const supabaseServer = createClient(process.env.SUPABASE_URL ?? "", process.env.SUPABASE_SERVICE_KEY ?? "")

const supabasePublic = createClient(process.env.SUPABASE_URL ?? "", process.env.SUPABASE_ANON_KEY ?? "")

export default { supabaseServer, supabasePublic }