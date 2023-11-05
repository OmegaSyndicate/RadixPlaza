import matplotlib.pyplot as plt
import matplotlib.dates as mdates
import pandas as pd
import numpy as np
import os

from scipy.optimize import minimize
from historic import BackTester, ExchangeState, Config, Shortage, bold_spines


if __name__ == "__main__":
    START_TIME = 1483225200
    END_TIME = 1577833200

    os.chdir('whitepaper/figures')
    data = pd.read_csv('ETHUSDT_1h.csv', skiprows=1).dropna().sort_values(by='unix')
    data['unix'] = data['unix'].astype('int') / 1000
    df = data[data['unix'] >= START_TIME]
    df = df[df['unix'] <= END_TIME]

    prev_len = 0
    while (prev_len != len(df)):
        prev_len = len(df)
        df_prev_shifted = df['close'].shift(1)
        df = df[~(df['close'] > 2 * df_prev_shifted)]
        print(f'Cleaning step -- initial {prev_len}, final {len(df)}')

    INITIAL_BASE = 50 / df.loc[df.index[0], 'close']
    INITIAL_QUOTE = 50

    config = Config(0.15, 20, 0.01, 0.998)                           
    initial_state = ExchangeState(
        INITIAL_BASE, INITIAL_QUOTE, INITIAL_BASE, INITIAL_QUOTE, df.loc[df.index[0], 'close'],
        df.loc[df.index[0], 'close'], df.loc[df.index[0], 'unix'], Shortage.BaseShortage
    )
    tester = BackTester(config, initial_state)

    def sortino_ratio(x, data):
        k_in, dilution, decay = x
        test_config = Config(k_in, k_in * 10**dilution, 0.01, 1-10**(-decay))
        tester.set_config(test_config)
        tester.reset()
        
        data = tester.run_experiment(data)
        daily = data.loc[::24].copy()
        daily['hodl'] = INITIAL_QUOTE + INITIAL_BASE * daily['close']
        daily['hodl_return'] = daily['hodl'] / daily['hodl'].shift()
        daily['value'] = daily['quote'] + daily['base'] * daily['close']
        daily['return'] = daily['value'] / daily['value'].shift()

        excess_return = (daily['return'] - daily['hodl_return']).dropna()
        downside = excess_return[excess_return < 0]
        downside_dev = np.sqrt(np.sum(downside**2) / len(excess_return))
        sortino = np.average(excess_return) / downside_dev * np.sqrt(365)
        print(f'tried x = {x}, sortino = {sortino}')
        return -sortino
    
    result = minimize(sortino_ratio, [0.2, 2, 2.7], (df), bounds=[(0.05, 2), (0, 2), (1, 4)])
    print(result.x)
    
    INITIAL_BASE = 50 / df.loc[df.index[-1], 'close']
    INITIAL_QUOTE = 50
    initial_state = ExchangeState(
        INITIAL_BASE, INITIAL_QUOTE, INITIAL_BASE, INITIAL_QUOTE, df.loc[df.index[-1], 'close'],
        df.loc[df.index[-1], 'close'], df.loc[df.index[-1], 'unix'], Shortage.BaseShortage
    )
    tester.initial_state = initial_state

    df = data[data['unix'] >= END_TIME].copy()
    #opt_config = Config(result.x[0], result.x[0] * 10**result.x[1], 0.01, 1-10**(-result.x[2]))
    opt_config = Config(1.75821287, 44.36427255, 0.01, 0.9998)
    uni_config = Config(1, 1, 0.03, 0)

    tester.set_config(uni_config)
    tester.reset()
    df = tester.run_experiment(df)
    df['UNIV2'] = df['quote'] + df['base'] * df['close']

    tester.set_config(opt_config)
    tester.reset()
    df = tester.run_experiment(df)
    df['CALM'] = df['quote'] + df['base'] * df['close']

    df['HODL'] = INITIAL_QUOTE + INITIAL_BASE * df['close']

    df['date'] = pd.to_datetime(df['date'])
    years = mdates.YearLocator()   
    years_fmt = mdates.DateFormatter('%Y')
    fig, ax = plt.subplots()
    ax.plot(df['date'], df['HODL'], label='HODL', color='k', linewidth=0.5)
    ax.plot(df['date'], df['CALM'], label='CALM', color='mediumseagreen', linewidth=0.5)
    ax.plot(df['date'], df['UNIV2'], label='UNIV2', color='steelblue', linewidth=0.5)
    ax.xaxis.set_major_locator(years) 
    ax.xaxis.set_major_formatter(years_fmt)
    ax.grid(True, which='major')
    legend = ax.legend()
    for line in legend.get_lines():
        line.set_linewidth(2.0)
    ax.set_xlim([df['date'].iloc[0], df['date'].iloc[-1]])
    ax.set_ylim([0, 1000])
    bold_spines()
    plt.savefig('eth_usd_opt.pdf')
    plt.close()