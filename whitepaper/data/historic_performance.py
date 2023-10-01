import os
import numpy as np
import pandas as pd
from numba import njit
from enum import Enum
from matplotlib import pyplot as plt

INITIAL_BTC = 1
INITIAL_USD = 8741
ARB_MARGIN = 0.003
START_TIME = 1483225200     # First of Jan 2017, BTC at $1000

concentration = 1                           
k_out = 1                            # 1 --> constant product
k_in = k_out / concentration           # calculate from concentration
fee = 0.003                             # trading fee
decay_factor = 0.9993                  # 1 day time constant
decay_factor = 0

class Shortage(Enum):
    BaseShortage = 0
    Equilibrium = 1
    QuoteShortage = 2

class ExhangeState:
    def __init__(self, BTC, USD, BTC_target, USD_target, p0, last_p_out, last_t_out, shortage):
        self.BTC = BTC
        self.USD = USD
        self.BTC_target = BTC_target
        self.USD_target = USD_target
        self.p0 = p0
        self.last_p_out = last_p_out
        self.last_t_out = last_t_out
        self.shortage = shortage

os.chdir('whitepaper/data')
df = pd.read_csv('bitcoin_1h.csv', skiprows=1).dropna().sort_values(by='unix')
df['unix'] = df['unix'].astype('int')
df = df[df['unix'] >= START_TIME]

df.loc[df.index[0], 'BTC'] = INITIAL_BTC
df.loc[df.index[0], 'USD'] = INITIAL_USD
df.loc[df.index[0], 'BTC_target'] = INITIAL_BTC
df.loc[df.index[0], 'USD_target'] = INITIAL_USD
df.loc[df.index[0], 'last_t_out'] = df.loc[df.index[0], 'unix']
df.loc[df.index[0], 'p0'] = df.loc[df.index[0], 'close']
df.loc[df.index[0], 'last_p_out'] = df.loc[df.index[0], 'p0']
df.loc[df.index[0], 'shortage'] = Shortage.BaseShortage

def new_state(state, timestamp, market_price):
    if state.shortage == Shortage.QuoteShortage:
        actual = state.USD
        target = state.USD_target
        shortfall = state.USD_target - state.USD
        surplus = state.BTC - state.BTC_target
        old_p_ref = 1 / state.p0
        p_market = 1 / market_price
        last_out_spot = 1 / state.last_p_out
    else:
        actual = state.BTC
        target = state.BTC_target
        shortfall = state.BTC_target - state.BTC
        surplus = state.USD - state.USD_target
        old_p_ref = state.p0
        p_market = market_price
        last_out_spot = state.last_p_out

    output_amount = 0
    delta_t = timestamp - state.last_t_out
    factor = decay_factor**(delta_t / 60)
    if shortfall != 0:
        p_ref_ss = surplus / shortfall / (1 + k_in * shortfall / actual)
    else:
        p_ref_ss = old_p_ref
    p_ref = factor * old_p_ref + (1 - factor) * p_ref_ss
    p_spot_in = ((1 - k_in) + k_in * (target / actual)**2) * p_ref
    #print(target, actual, shortfall, surplus)
    #print(old_p_ref, p_ref_ss, p_ref, factor)

    # Inward trade
    if p_market * (1 + fee) * (1 + ARB_MARGIN) < p_spot_in:
        radicand = 1 + 4 * k_in * surplus / p_ref / actual
        adjusted_target = ((2 * k_in + np.sqrt(radicand) - 1) / k_in / 2) * actual

        # Reaches equilibrium
        if p_market <= p_ref:
            if state.shortage == Shortage.QuoteShortage:
                state.USD = adjusted_target
                state.USD_target = adjusted_target
                state.BTC = state.BTC_target
                state.p0 = 1 / p_ref
                state.last_p_out = 1 / p_ref
                target = state.BTC_target
                actual = state.BTC                
            else:
                state.BTC = adjusted_target
                state.BTC_target = adjusted_target
                state.USD = state.USD_target
                state.p0 = p_ref
                state.last_p_out = p_ref
                target = state.USD_target
                actual = state.USD
            state.shortage = Shortage.Equilibrium
            output_amount = surplus
            shortfall = 0
            surplus = 0

            p_ref_ss = 1 / p_ref
            p_market = 1 / p_market
            last_out_spot = 1 / p_ref
            p_ref = 1 / p_ref
        else:
            p_stop = p_market * (1 + fee)
            new_actual = np.sqrt(k_in / (p_stop / p_ref + (k_in - 1))) * adjusted_target
            new_surplus = (
                (1 - 2 * k_in)
                + (k_in - 1) * np.sqrt(k_in / (p_stop / p_ref + (k_in - 1)))
                + k_in * np.sqrt((p_stop / p_ref + (k_in - 1)) / k_in)
            ) * p_ref * adjusted_target
            output_amount = surplus - new_surplus
            
            if state.shortage == Shortage.QuoteShortage:
                state.USD = new_actual
                state.BTC = state.BTC_target + new_surplus + output_amount * fee
                state.BTC_target += output_amount * fee
                state.USD_target = (
                    2 * k_in
                    + np.sqrt(1 + 4 * k_in * new_surplus / p_ref / new_actual)
                    - 1
                ) / (2 * k_in) * new_actual
            else:
                state.BTC = new_actual
                state.USD = state.USD_target + new_surplus + output_amount * fee
                state.USD_target += output_amount * fee
                state.BTC_target = (
                    2 * k_in
                    + np.sqrt(1 + 4 * k_in * new_surplus / p_ref / new_actual)
                    - 1
                ) / (2 * k_in) * new_actual
    
    # Outward trade
    p_spot_ss = (1 + k_in * (target**2 - actual**2) / actual**2) * p_ref
    p_spot = factor * last_out_spot + (1 - factor) * p_spot_ss
    if p_market > (1 + fee) * (1 + ARB_MARGIN) * p_spot or state.shortage == Shortage.Equilibrium:
        #print(target, actual, state.p0)
        #print(p_market, p_ref, p_spot_ss, p_spot, last_out_spot)
        virtual_p_ref = p_spot / (1 + k_out * (target**2 - actual**2) / actual**2)
        p_stop = p_market / (1 + fee)
        new_actual = np.sqrt(k_out / (p_stop / virtual_p_ref + (k_out - 1))) * target
        new_surplus = (
            (1 - 2 * k_out)
            + (k_out - 1) * np.sqrt(k_out / (p_stop / virtual_p_ref + (k_out - 1)))
            + k_out * np.sqrt((p_stop / virtual_p_ref + (k_out - 1)) / k_out)
        ) * virtual_p_ref * target
        old_surplus = (
            (1 - 2 * k_out)
            + (k_out - 1) * np.sqrt(k_out / (p_spot / virtual_p_ref + (k_out - 1)))
            + k_out * np.sqrt((p_spot / virtual_p_ref + (k_out - 1)) / k_out)
        ) * virtual_p_ref * target
        output_amount += actual - new_actual

        state.last_t_out = timestamp
        if market_price > state.p0:
            state.shortage = Shortage.BaseShortage
            state.BTC = new_actual + output_amount * fee
            state.BTC_target += output_amount * fee
            state.USD += new_surplus - old_surplus
            state.p0 = p_ref
            state.last_p_out = p_stop
        else:
            state.shortage = Shortage.QuoteShortage
            state.USD = new_actual + output_amount * fee
            state.USD_target += output_amount * fee
            state.BTC += new_surplus - old_surplus
            state.p0 = 1 / p_ref
            state.last_p_out = 1 / p_stop
            
    return state

#@njit
def run_experiment():
    variables = ['BTC', 'USD', 'BTC_target', 'USD_target', 'p0', 'last_p_out', 'last_t_out', 'shortage']
    state = df.loc[df.index[0], variables]
    index = df.index
    for i in range(1, len(df)):
        ix = index[i]
        timestamp = df.loc[ix, 'unix']
        market_price = df.loc[ix, 'close']
        state = new_state(state, timestamp, market_price)
        # df.loc[ix, variables] = state
    return state

print(run_experiment())