export type Json =
  | string
  | number
  | boolean
  | null
  | { [key: string]: Json | undefined }
  | Json[]

export type Database = {
  public: {
    Tables: {
      "Sfacg-Accounts": {
        Row: {
          accountId: number | null
          avatar: string | null
          cookie: string | null
          couponsRemain: number | null
          fireMoneyRemain: number | null
          lastCheckIn: string | null
          nickName: string | null
          passWord: string
          userName: string
          vipLevel: number | null
        }
        Insert: {
          accountId?: number | null
          avatar?: string | null
          cookie?: string | null
          couponsRemain?: number | null
          fireMoneyRemain?: number | null
          lastCheckIn?: string | null
          nickName?: string | null
          passWord: string
          userName: string
          vipLevel?: number | null
        }
        Update: {
          accountId?: number | null
          avatar?: string | null
          cookie?: string | null
          couponsRemain?: number | null
          fireMoneyRemain?: number | null
          lastCheckIn?: string | null
          nickName?: string | null
          passWord?: string
          userName?: string
          vipLevel?: number | null
        }
        Relationships: []
      }
      "Sfacg-chapter": {
        Row: {
          chapId: number
          content: string | null
          isVip: boolean | null
          needFireMoney: number | null
          ntitle: string | null
          volumeId: number | null
        }
        Insert: {
          chapId?: number
          content?: string | null
          isVip?: boolean | null
          needFireMoney?: number | null
          ntitle?: string | null
          volumeId?: number | null
        }
        Update: {
          chapId?: number
          content?: string | null
          isVip?: boolean | null
          needFireMoney?: number | null
          ntitle?: string | null
          volumeId?: number | null
        }
        Relationships: [
          {
            foreignKeyName: "public_Sfacg-chapter_volumeId_fkey"
            columns: ["volumeId"]
            isOneToOne: false
            referencedRelation: "Sfacg-volumeInfos"
            referencedColumns: ["volumeId"]
          },
        ]
      }
      "Sfacg-novelInfo": {
        Row: {
          authorName: string | null
          charCount: string | null
          intro: string | null
          isFinish: boolean | null
          lastUpdateTime: string | null
          novelCover: string | null
          novelId: number
          novelName: string | null
          tags: string | null
        }
        Insert: {
          authorName?: string | null
          charCount?: string | null
          intro?: string | null
          isFinish?: boolean | null
          lastUpdateTime?: string | null
          novelCover?: string | null
          novelId?: number
          novelName?: string | null
          tags?: string | null
        }
        Update: {
          authorName?: string | null
          charCount?: string | null
          intro?: string | null
          isFinish?: boolean | null
          lastUpdateTime?: string | null
          novelCover?: string | null
          novelId?: number
          novelName?: string | null
          tags?: string | null
        }
        Relationships: []
      }
      "Sfacg-volumeInfos": {
        Row: {
          novelId: number | null
          title: string | null
          volumeId: number
        }
        Insert: {
          novelId?: number | null
          title?: string | null
          volumeId?: number
        }
        Update: {
          novelId?: number | null
          title?: string | null
          volumeId?: number
        }
        Relationships: [
          {
            foreignKeyName: "public_Sfacg-volumeInfos_novelId_fkey"
            columns: ["novelId"]
            isOneToOne: false
            referencedRelation: "Sfacg-novelInfo"
            referencedColumns: ["novelId"]
          },
        ]
      }
    }
    Views: {
      [_ in never]: never
    }
    Functions: {
      [_ in never]: never
    }
    Enums: {
      [_ in never]: never
    }
    CompositeTypes: {
      [_ in never]: never
    }
  }
}

type PublicSchema = Database[Extract<keyof Database, "public">]

export type Tables<
  PublicTableNameOrOptions extends
    | keyof (PublicSchema["Tables"] & PublicSchema["Views"])
    | { schema: keyof Database },
  TableName extends PublicTableNameOrOptions extends { schema: keyof Database }
    ? keyof (Database[PublicTableNameOrOptions["schema"]]["Tables"] &
        Database[PublicTableNameOrOptions["schema"]]["Views"])
    : never = never,
> = PublicTableNameOrOptions extends { schema: keyof Database }
  ? (Database[PublicTableNameOrOptions["schema"]]["Tables"] &
      Database[PublicTableNameOrOptions["schema"]]["Views"])[TableName] extends {
      Row: infer R
    }
    ? R
    : never
  : PublicTableNameOrOptions extends keyof (PublicSchema["Tables"] &
        PublicSchema["Views"])
    ? (PublicSchema["Tables"] &
        PublicSchema["Views"])[PublicTableNameOrOptions] extends {
        Row: infer R
      }
      ? R
      : never
    : never

export type TablesInsert<
  PublicTableNameOrOptions extends
    | keyof PublicSchema["Tables"]
    | { schema: keyof Database },
  TableName extends PublicTableNameOrOptions extends { schema: keyof Database }
    ? keyof Database[PublicTableNameOrOptions["schema"]]["Tables"]
    : never = never,
> = PublicTableNameOrOptions extends { schema: keyof Database }
  ? Database[PublicTableNameOrOptions["schema"]]["Tables"][TableName] extends {
      Insert: infer I
    }
    ? I
    : never
  : PublicTableNameOrOptions extends keyof PublicSchema["Tables"]
    ? PublicSchema["Tables"][PublicTableNameOrOptions] extends {
        Insert: infer I
      }
      ? I
      : never
    : never

export type TablesUpdate<
  PublicTableNameOrOptions extends
    | keyof PublicSchema["Tables"]
    | { schema: keyof Database },
  TableName extends PublicTableNameOrOptions extends { schema: keyof Database }
    ? keyof Database[PublicTableNameOrOptions["schema"]]["Tables"]
    : never = never,
> = PublicTableNameOrOptions extends { schema: keyof Database }
  ? Database[PublicTableNameOrOptions["schema"]]["Tables"][TableName] extends {
      Update: infer U
    }
    ? U
    : never
  : PublicTableNameOrOptions extends keyof PublicSchema["Tables"]
    ? PublicSchema["Tables"][PublicTableNameOrOptions] extends {
        Update: infer U
      }
      ? U
      : never
    : never

export type Enums<
  PublicEnumNameOrOptions extends
    | keyof PublicSchema["Enums"]
    | { schema: keyof Database },
  EnumName extends PublicEnumNameOrOptions extends { schema: keyof Database }
    ? keyof Database[PublicEnumNameOrOptions["schema"]]["Enums"]
    : never = never,
> = PublicEnumNameOrOptions extends { schema: keyof Database }
  ? Database[PublicEnumNameOrOptions["schema"]]["Enums"][EnumName]
  : PublicEnumNameOrOptions extends keyof PublicSchema["Enums"]
    ? PublicSchema["Enums"][PublicEnumNameOrOptions]
    : never
