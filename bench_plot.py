import pandas as pd
import matplotlib.pyplot as plt
import sys


def main():
    df = pd.read_csv('bench_rluset.csv')
    
    # Group by write ratio
    grouped = df.groupby('Write_Ratio')
    
    # Plot for each group
    for name, group in grouped:
        plt.plot(group['Thread_Count'], group['Throughput'], label=f'Write Ratio {name}')

    # Add legend
    plt.legend()

    # Set labels and title
    plt.xlabel('Thread Count')
    plt.ylabel('Throughput')
    plt.title('RluSet Benchmark')

    # Show plot
    plt.savefig('bench_rluset.png')

if __name__ == "__main__":
    main()




