import os
import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.dates as mdates
import matplotlib as mpl

from historic import Config, ExchangeState, BackTester, Shortage, bold_spines

INITIAL_ETH = 10 * 500 / 1292
INITIAL_USD = 500
START_TIME = 1577833200     # First of Jan 2020, ETH at $129.19

os.chdir('whitepaper/data')
df = pd.read_csv('ETHUSDC_1h.csv', skiprows=1).dropna().sort_values(by='unix')
df['unix'] = df['unix'].astype('int') / 1000
df = df[df['unix'] >= START_TIME]

k_in = 0.15
dilution = 20/.15
calm_config = Config(k_in, k_in*dilution, 0.01, 0.998)                           

initial_state = ExchangeState(
    INITIAL_ETH, INITIAL_USD, INITIAL_ETH, INITIAL_USD, df.loc[df.index[0], 'close'],
    df.loc[df.index[0], 'close'], df.loc[df.index[0], 'unix'], Shortage.BaseShortage
)

tester = BackTester(calm_config, initial_state)
df = tester.run_experiment(df)

df['HODL'] = INITIAL_USD + INITIAL_ETH * df['close']
df['CALM'] = df['quote'] + df['base'] * df['close']

returns = {}
returns['calm\nETH/USD'] = tester.state.base * df.loc[df.index[-1], 'close'] + tester.state.quote
if tester.state.shortage == Shortage.BaseShortage:
    returns['calm\nUSD'] = 2 * tester.state.quote_target
    returns['calm\nETH'] = 2 * (tester.state.base * df.loc[df.index[-1], 'close'] + tester.state.quote - tester.state.quote_target)
else:
    returns['calm\nUSD'] = 2 * tester.state.quote + (tester.state.base - tester.state.base_target) * df.loc[df.index[-1], 'close']
    returns['calm\nETH'] = 2 * tester.state.base_target * df.loc[df.index[-1], 'close']

print(pd.Series(vars(tester.state)))

xyk_config = Config(1, 1, 0.003, 0)
tester.set_config(xyk_config)
tester.reset()
df = tester.run_experiment(df)
df['UNIV2'] = df['quote'] + df['base'] * df['close']

df['date'] = pd.to_datetime(df['date'])
years = mdates.YearLocator()   
years_fmt = mdates.DateFormatter('%Y')

fig, ax = plt.subplots(figsize=(8, 4.8))
ax.plot(df['date'], df['HODL'], label='HODL', color='dimgray', linewidth=0.5)
ax.plot(df['date'], df['UNIV2'], label='UNIV2', color='steelblue', linewidth=0.5)
ax.plot(df['date'], df['CALM'], label='CALM', color='mediumseagreen', linewidth=0.5)
ax.xaxis.set_major_locator(years) 
ax.xaxis.set_major_formatter(years_fmt)
ax.grid(True, which='major')
legend = ax.legend()
for l in legend.get_lines():
    l.set_linewidth(1.5)
ax.set_xlim([df['date'].iloc[0], df['date'].iloc[-1]])
ax.set_ylim([0, 22500])
ax.set_ylabel('portfolio value [$]', weight='bold')
bold_spines()
plt.savefig('ETH_USD.pdf')
plt.close()

returns['hodl\nUSD'] = 2 * INITIAL_USD
returns['hodl\nETH'] = 2 * INITIAL_USD * df.loc[df.index[-1], 'close'] / df.loc[df.index[0], 'close']
returns['univ2\nETH/USD'] = tester.state.base * df.loc[df.index[-1], 'close'] + tester.state.quote
returns = pd.Series(returns).sort_values()

print(returns)

fig, ax = plt.subplots(figsize=(8, 4.8))
returns.plot(kind='bar', color='steelblue')
plt.xticks(rotation=0, ha='center', fontsize=12)
ax.set_ylim([0, 15000])
ax.set_ylabel('Final portfolio value [$]', weight='bold')
ax.grid(True, which='major')
bold_spines()
plt.tight_layout()
plt.savefig('returns.pdf')
plt.close()
