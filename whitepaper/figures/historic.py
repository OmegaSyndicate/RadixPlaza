import os
import numpy as np
import pandas as pd
from enum import Enum
from copy import copy
import matplotlib.pyplot as plt
import matplotlib.dates as mdates
import matplotlib as mpl

mpl.rcParams['font.weight'] = 'bold'
ARB_MARGIN = 0.003

def bold_spines():
    ax = plt.gca()
    for spine in ax.spines.values():
        spine.set_linewidth(2)
    return ax

class Shortage(Enum):
    BaseShortage = 0
    Equilibrium = 1
    QuoteShortage = 2

class Config():
    def __init__(self, k_in, k_out, fee, decay_factor):
        self.k_in = k_in
        self.k_out = k_out
        self.fee = fee
        self.decay_factor = decay_factor

class ExchangeState:
    def __init__(self, base, quote, base_target, quote_target, p0, last_p_out, last_t_out, shortage):
        self.base = base
        self.quote = quote
        self.base_target = base_target
        self.quote_target = quote_target
        self.p0 = p0
        self.last_p_out = last_p_out
        self.last_t_out = last_t_out
        self.shortage = shortage

class BackTester():
    def __init__(self, config, initial_state):
        self.set_config(config)
        self.initial_state = initial_state
        self.reset()

    def reset(self):
        self.state = copy(self.initial_state)

    def set_config(self, config):
        self.config = config

    def step(self, timestamp, market_price):
        old_state = copy(self.state)
        if self.state.shortage == Shortage.QuoteShortage:
            actual = self.state.quote
            target = self.state.quote_target
            shortfall = self.state.quote_target - self.state.quote
            surplus = self.state.base - self.state.base_target
            old_p_ref = 1 / self.state.p0
            p_market = 1 / market_price
            last_out_spot = 1 / self.state.last_p_out
        else:
            actual = self.state.base
            target = self.state.base_target
            shortfall = self.state.base_target - self.state.base
            surplus = self.state.quote - self.state.quote_target
            old_p_ref = self.state.p0
            p_market = market_price
            last_out_spot = self.state.last_p_out

        output_amount = 0
        delta_t = timestamp - self.state.last_t_out
        if self.config.decay_factor > 0:
            factor = self.config.decay_factor**(delta_t / 60)
        else:
            factor = 0
        if shortfall != 0:
            p_ref_ss = surplus / shortfall / (1 + self.config.k_in * shortfall / actual)
        else:
            p_ref_ss = old_p_ref
        p_ref = factor * old_p_ref + (1 - factor) * p_ref_ss
        p_spot_in = ((1 - self.config.k_in) + self.config.k_in * (target / actual)**2) * p_ref

        # Inward trade
        if p_market * (1 + ARB_MARGIN) < (1 - self.config.fee) * p_spot_in:
            radicand = 1 + 4 * self.config.k_in * surplus / p_ref / actual
            adjusted_target = ((2 * self.config.k_in + np.sqrt(radicand) - 1) / self.config.k_in / 2) * actual

            # Reaches equilibrium
            p_stop = p_market / (1 - self.config.fee)
            if p_stop < p_ref:
                if self.state.shortage == Shortage.QuoteShortage:
                    self.state.quote = adjusted_target
                    self.state.quote_target = adjusted_target
                    self.state.base = self.state.base_target
                    self.state.p0 = 1 / p_ref
                    self.state.last_p_out = 1 / p_ref
                    target = self.state.base_target
                    actual = self.state.base                
                else:
                    self.state.base = adjusted_target
                    self.state.base_target = adjusted_target
                    self.state.quote = self.state.quote_target
                    self.state.p0 = p_ref
                    self.state.last_p_out = p_ref
                    target = self.state.quote_target
                    actual = self.state.quote
                self.state.shortage = Shortage.Equilibrium
                output_amount = surplus
                shortfall = 0
                surplus = 0

                p_ref_ss = 1 / p_ref
                p_market = 1 / p_market
                last_out_spot = 1 / p_ref
                p_ref = 1 / p_ref
            else:
                new_actual = \
                    np.sqrt(self.config.k_in / (p_stop / p_ref + (self.config.k_in - 1))) * adjusted_target
                new_surplus = (
                    (1 - 2 * self.config.k_in)
                    + (self.config.k_in - 1) * np.sqrt(self.config.k_in / (p_stop / p_ref + (self.config.k_in - 1)))
                    + self.config.k_in * np.sqrt((p_stop / p_ref + (self.config.k_in - 1)) / self.config.k_in)
                ) * p_ref * adjusted_target
                output_amount = surplus - new_surplus
                
                if self.state.shortage == Shortage.QuoteShortage:
                    self.state.quote = new_actual
                    self.state.base = self.state.base_target + new_surplus + output_amount * self.config.fee
                    self.state.base_target += output_amount * self.config.fee
                    self.state.quote_target = (
                        2 * self.config.k_in
                        + np.sqrt(1 + 4 * self.config.k_in * new_surplus / p_ref_ss / new_actual)
                        - 1
                    ) / (2 * self.config.k_in) * new_actual
                else:
                    self.state.base = new_actual
                    self.state.quote = self.state.quote_target + new_surplus + output_amount * self.config.fee
                    self.state.quote_target += output_amount * self.config.fee
                    self.state.base_target = (
                        2 * self.config.k_in
                        + np.sqrt(1 + 4 * self.config.k_in * new_surplus / p_ref_ss / new_actual)
                        - 1
                    ) / (2 * self.config.k_in) * new_actual
        
        # Outward trade
        p_spot_ss = (1 + self.config.k_in * (target**2 - actual**2) / actual**2) * p_ref
        p_spot = factor * last_out_spot + (1 - factor) * p_spot_ss
        if p_market * (1 - self.config.fee) > (1 + ARB_MARGIN) * p_spot or self.state.shortage == Shortage.Equilibrium:
            virtual_p_ref = p_spot / (1 + self.config.k_out * (target**2 - actual**2) / actual**2)
            p_stop = p_market / (1 + self.config.fee)
            new_actual = np.sqrt(self.config.k_out / (p_stop / virtual_p_ref + (self.config.k_out - 1))) * target
            new_surplus = (
                (1 - 2 * self.config.k_out)
                + (self.config.k_out - 1) * np.sqrt(self.config.k_out / (p_stop / virtual_p_ref + (self.config.k_out - 1)))
                + self.config.k_out * np.sqrt((p_stop / virtual_p_ref + (self.config.k_out - 1)) / self.config.k_out)
            ) * virtual_p_ref * target
            old_surplus = (
                (1 - 2 * self.config.k_out)
                + (self.config.k_out - 1) * np.sqrt(self.config.k_out / (p_spot / virtual_p_ref + (self.config.k_out - 1)))
                + self.config.k_out * np.sqrt((p_spot / virtual_p_ref + (self.config.k_out - 1)) / self.config.k_out)
            ) * virtual_p_ref * target
            output_amount += actual - new_actual

            self.state.last_t_out = timestamp
            if market_price > self.state.p0:
                self.state.shortage = Shortage.BaseShortage
                self.state.base = new_actual + output_amount * self.config.fee
                self.state.base_target += output_amount * self.config.fee
                self.state.quote += new_surplus - old_surplus
                self.state.p0 = p_ref
                self.state.last_p_out = p_stop
            else:
                self.state.shortage = Shortage.QuoteShortage
                self.state.quote = new_actual + output_amount * self.config.fee
                self.state.quote_target += output_amount * self.config.fee
                self.state.base += new_surplus - old_surplus
                self.state.p0 = 1 / p_ref
                self.state.last_p_out = 1 / p_stop
        if self.state.quote > self.state.quote_target and self.state.shortage == Shortage.QuoteShortage:
            print(f'\n\n\n\n {pd.Series(vars(old_state))} \n {delta_t} \n {p_ref} \n {market_price} \n {pd.Series(vars(self.state))} \n\n\n\n')
            raise Exception("Oops")


    def run_experiment(self, data):
        variables = ['base', 'quote', 'base_target', 'quote_target', 'p0', 'last_p_out', 'last_t_out', 'shortage']
        index = data.index
        for i in range(len(data)):
            ix = index[i]
            timestamp = data.loc[ix, 'unix']
            market_price = data.loc[ix, 'close']
            self.step(timestamp, market_price)
            data.loc[ix, variables] = vars(self.state).values()
        return data


if __name__ == "__main__":
    INITIAL_BASE = 3.87
    INITIAL_QUOTE = 500
    START_TIME = 1577833200

    os.chdir('whitepaper/figures')
    df = pd.read_csv('ETHUSDT_1h.csv', skiprows=1).dropna().sort_values(by='unix')
    df['unix'] = df['unix'].astype('int') / 1000
    df = df[df['unix'] >= START_TIME]

    config = Config(4.74957491, 474.957491, 0.1, 0.9998519972)
    initial_state = ExchangeState(
        INITIAL_BASE, INITIAL_QUOTE, INITIAL_BASE, INITIAL_QUOTE, df.loc[df.index[0], 'close'],
        df.loc[df.index[0], 'close'], df.loc[df.index[0], 'unix'], Shortage.BaseShortage
    )

    tester = BackTester(config, initial_state)
    df = tester.run_experiment(df)

    df['HODL'] = INITIAL_QUOTE + INITIAL_BASE * df['close']
    df['CALM'] = df['quote'] + df['base'] * df['close']
    print(pd.Series(vars(tester.state)))

    config = Config(1, 1, 0.003, 0)
    tester.set_config(config)
    tester.reset()
    df = tester.run_experiment(df)
    df['UNIV2'] = df['quote'] + df['base'] * df['close']

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
    ax.legend()
    ax.set_xlim([df['date'].iloc[0], df['date'].iloc[-1]])
    ax.set_ylim([0, 22500])
    bold_spines()
    plt.savefig('eth_usdt.pdf')
    plt.close()