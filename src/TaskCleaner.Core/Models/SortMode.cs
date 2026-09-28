namespace TaskCleaner.Core.Models
{
    public enum SortMode
    {
        Composite = 0,
        Memory = 1,
        Cpu = 2,
        Windows = 3,
        Default = 4
    }

    public static class SortModeExtensions
    {
        public static string GetDisplayName(this SortMode mode)
        {
            return mode switch
            {
                SortMode.Composite => "综合负载",
                SortMode.Memory => "内存占用",
                SortMode.Cpu => "CPU 占用",
                SortMode.Windows => "窗口数量",
                SortMode.Default => "默认顺序",
                _ => "综合负载"
            };
        }
    }
}
