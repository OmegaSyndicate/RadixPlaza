import matplotlib.pyplot as plt
import matplotlib.dates as mdates
import pandas as pd
import os

from scipy.optimize import minimize
from historic import BackTester, ExchangeState, Config, Shortage, bold_spines


if __name__ == "__main__":
    INITIAL_BASE = 500/8.06
    INITIAL_QUOTE = 500/998.05
    START_TIME = 1483225200
    END_TIME = 1577833200

    os.chdir('whitepaper/figures')
    df = pd.read_csv('ETHBTC_1h.csv', skiprows=1).dropna().sort_values(by='unix')
    df['unix'] = df['unix'].astype('int')
    df = df[df['unix'] >= START_TIME]
    #df = df[df['unix'] <= END_TIME]

    prev_len = 0
    while (prev_len != len(df)):
        prev_len = len(df)
        df_prev_shifted = df['close'].shift(1)
        df = df[~(df['close'] > 2 * df_prev_shifted)]
        print(f'Cleaning step -- initial {prev_len}, final {len(df)}')

    config = Config(0.15, 20, 0.01, 0.998)                           
    initial_state = ExchangeState(
        INITIAL_BASE, INITIAL_QUOTE, INITIAL_BASE, INITIAL_QUOTE, df.loc[df.index[0], 'close'],
        df.loc[df.index[0], 'close'], df.loc[df.index[0], 'unix'], Shortage.BaseShortage
    )
    tester = BackTester(config, initial_state)

    def final_value(x, data):
        k_in, dilution, decay = x
        test_config = Config(k_in, k_in * 10**dilution, 0.01, 1-10**(-decay))
        tester.set_config(test_config)
        tester.reset()
        
        data = tester.run_experiment(data)
        last_t = data.index[-1]
        value = data.loc[last_t, 'quote'] + data.loc[last_t, 'base'] * data.loc[last_t, 'close']
        print(f'tried x = {x}, value = {value}')
        return -value
    
    result = minimize(final_value, [0.7, 0.6, 3], (df), bounds=[(0.05,2), (0, 2), (1, 4)])
    print(result.x)

    # df['HODL'] = INITIAL_QUOTE + INITIAL_BASE * df['close']
    # df['CALM'] = df['quote'] + df['base'] * df['close']
    # print(pd.Series(vars(tester.state)))

    # config = Config(1, 1, 0.003, 0)
    # tester.set_config(config)
    # tester.reset()
    # df = tester.run_experiment(df)
    # df['UNIV2'] = df['quote'] + df['base'] * df['close']

    # df['date'] = pd.to_datetime(df['date'])
    # years = mdates.YearLocator()   
    # years_fmt = mdates.DateFormatter('%Y')
    # fig, ax = plt.subplots()
    # ax.plot(df['date'], df['HODL'], label='HODL', color='k', linewidth=0.5)
    # ax.plot(df['date'], df['CALM'], label='CALM', color='mediumseagreen', linewidth=0.5)
    # ax.plot(df['date'], df['UNIV2'], label='UNIV2', color='steelblue', linewidth=0.5)
    # ax.xaxis.set_major_locator(years) 
    # ax.xaxis.set_major_formatter(years_fmt)
    # ax.grid(True, which='major')
    # ax.legend()
    # ax.set_xlim([df['date'].iloc[0], df['date'].iloc[-1]])
    # ax.set_ylim([0, 22500])
    # bold_spines()
    # plt.savefig('eth_usdt.pdf')
    # plt.close()