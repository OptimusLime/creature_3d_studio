// Generate random sequences for verification against Rust implementation
// Compile: dotnet build -o .
// Run: dotnet run

using System;
using System.IO;

class Program
{
    static void Main(string[] args)
    {
        // Test multiple seeds
        int[] seeds = { 0, 1, 42, 12345, -1, int.MinValue, int.MaxValue };
        
        foreach (int seed in seeds)
        {
            string filename = $"random_sequence_seed_{seed}.txt";
            using (StreamWriter writer = new StreamWriter(filename))
            {
                Random rng = new Random(seed);
                
                // Write header
                writer.WriteLine($"# .NET Random sequence for seed {seed}");
                writer.WriteLine($"# Format: method,args,result");
                writer.WriteLine();
                
                // Generate Next() calls
                writer.WriteLine("# Next() - 100 calls");
                for (int i = 0; i < 100; i++)
                {
                    writer.WriteLine($"Next,{rng.Next()}");
                }
                writer.WriteLine();
                
                // Generate Next(maxValue) calls
                writer.WriteLine("# Next(maxValue) - various maxValues");
                Random rng2 = new Random(seed);
                int[] maxValues = { 1, 2, 10, 100, 1000, 10000, int.MaxValue };
                foreach (int maxValue in maxValues)
                {
                    for (int i = 0; i < 20; i++)
                    {
                        writer.WriteLine($"Next,{maxValue},{rng2.Next(maxValue)}");
                    }
                }
                writer.WriteLine();
                
                // Generate Next(minValue, maxValue) calls
                writer.WriteLine("# Next(minValue, maxValue)");
                Random rng3 = new Random(seed);
                (int min, int max)[] ranges = { (0, 10), (5, 15), (-10, 10), (0, 100), (1000, 2000) };
                foreach (var (min, max) in ranges)
                {
                    for (int i = 0; i < 20; i++)
                    {
                        writer.WriteLine($"Next,{min},{max},{rng3.Next(min, max)}");
                    }
                }
                writer.WriteLine();
                
                // Generate NextDouble() calls
                writer.WriteLine("# NextDouble() - 50 calls");
                Random rng4 = new Random(seed);
                for (int i = 0; i < 50; i++)
                {
                    writer.WriteLine($"NextDouble,{rng4.NextDouble():R}");
                }
            }
            
            Console.WriteLine($"Generated {filename}");
        }
        
        // Generate a single comprehensive test file with seed 42
        using (StreamWriter writer = new StreamWriter("random_test_seed_42.txt"))
        {
            Random rng = new Random(42);
            
            writer.WriteLine("# Comprehensive test for seed 42");
            writer.WriteLine("# Each line: result value");
            writer.WriteLine();
            
            // 1000 Next() calls
            writer.WriteLine("# SECTION: Next() x 1000");
            for (int i = 0; i < 1000; i++)
            {
                writer.WriteLine(rng.Next());
            }
            
            // Reset and do Next(maxValue) calls
            writer.WriteLine();
            writer.WriteLine("# SECTION: Next(100) x 100 (fresh seed)");
            rng = new Random(42);
            for (int i = 0; i < 100; i++)
            {
                writer.WriteLine(rng.Next(100));
            }
            
            // Reset and do NextDouble() calls  
            writer.WriteLine();
            writer.WriteLine("# SECTION: NextDouble() x 100 (fresh seed)");
            rng = new Random(42);
            for (int i = 0; i < 100; i++)
            {
                writer.WriteLine($"{rng.NextDouble():R}");
            }
        }
        
        Console.WriteLine("Generated random_test_seed_42.txt");
    }
}
