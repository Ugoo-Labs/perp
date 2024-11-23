import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface Asset { 'class' : AssetClass, 'symbol' : string }
export type AssetClass = { 'Cryptocurrency' : null } |
  { 'FiatCurrency' : null };
export interface LimitOrder {
  'buy' : boolean,
  'init_lower_bound' : bigint,
  'init_removed_liquidity' : bigint,
  'init_tick_timestamp' : bigint,
  'order_size' : bigint,
  'ref_tick' : bigint,
}
export interface LiquidityBoundary {
  'upper_bound' : bigint,
  'lower_bound' : bigint,
  'lifetime_removed_liquidity' : bigint,
}
export interface MarketDetails {
  'vault_id' : Principal,
  'collateral_decimal' : number,
  'quote_asset' : Asset,
  'base_asset' : Asset,
  'xrc_id' : Principal,
}
export type OrderType = { 'Limit' : null } |
  { 'Market' : null };
export interface PositionDetails {
  'debt_value' : bigint,
  'long' : boolean,
  'entry_tick' : bigint,
  'order_type' : PositionOrderType,
  'timestamp' : bigint,
  'interest_rate' : number,
  'collateral_value' : bigint,
  'volume_share' : bigint,
}
export type PositionOrderType = { 'Limit' : LimitOrder } |
  { 'Market' : null };
export type Result = { 'Ok' : PositionDetails } |
  { 'Err' : string };
export interface StateDetails {
  'max_leveragex10' : number,
  'not_paused' : boolean,
  'current_tick' : bigint,
  'base_token_multiple' : number,
  'min_collateral' : bigint,
}
export interface TickDetails {
  'liq_bounds_token0' : LiquidityBoundary,
  'liq_bounds_token1' : LiquidityBoundary,
  'created_timestamp' : bigint,
}
export interface _SERVICE {
  'closePosition' : ActorMethod<[[] | [bigint]], bigint>,
  'getAccountPosition' : ActorMethod<[Uint8Array | number[]], PositionDetails>,
  'getBestOfferTick' : ActorMethod<[boolean], bigint>,
  'getMarketDetails' : ActorMethod<[], MarketDetails>,
  'getPositionPNL' : ActorMethod<[PositionDetails], bigint>,
  'getStateDetails' : ActorMethod<[], StateDetails>,
  'getTickDetails' : ActorMethod<[bigint], TickDetails>,
  'getUserAccount' : ActorMethod<[Principal], Uint8Array | number[]>,
  'liquidatePosition' : ActorMethod<[Principal], undefined>,
  'openPosition' : ActorMethod<
    [bigint, boolean, OrderType, number, [] | [bigint], bigint, bigint],
    Result
  >,
  'positionStatus' : ActorMethod<[Uint8Array | number[]], [boolean, boolean]>,
  'retryAccountError' : ActorMethod<[Principal], undefined>,
  'startTimer' : ActorMethod<[], undefined>,
  'successNotification' : ActorMethod<
    [Uint8Array | number[], bigint],
    undefined
  >,
  'updateStateDetails' : ActorMethod<[StateDetails], undefined>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
