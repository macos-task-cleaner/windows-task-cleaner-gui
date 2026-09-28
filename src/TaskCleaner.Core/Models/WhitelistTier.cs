namespace TaskCleaner.Core.Models
{
    public enum WhitelistTier
    {
        None = 0,
        L1CoreSystem = 1,
        L2CallerContext = 2,
        L3PersistentUtilities = 3,
        L4UserConfig = 4
    }

    public static class WhitelistTierExtensions
    {
        public static string GetLocalizedName(this WhitelistTier tier)
        {
            return tier switch
            {
                WhitelistTier.L1CoreSystem => "L1 核心系统",
                WhitelistTier.L2CallerContext => "L2 终端环境",
                WhitelistTier.L3PersistentUtilities => "L3 系统设施",
                WhitelistTier.L4UserConfig => "L4 用户配置",
                _ => string.Empty
            };
        }
    }
}
