
extern crate time;
extern crate uuid;

use std::string::ToString;
use std::collections::HashMap;

use time::{strftime};
use uuid::Uuid;


/// 货币种类: 人民币
pub const CURRENCY_CNY: &'static str = "CNY";
/// 统一下单
pub const UNIFIEDORDER_URL: &'static str = "https://api.mch.weixin.qq.com/pay/unifiedorder";
/// 查询订单
pub const ORDERQUERY_URL: &'static str = "https://api.mch.weixin.qq.com/pay/orderquery";


/// [交易类型]
/// APP--app支付，统一下单接口trade_type的传参可参考这里
pub enum TradeType {
    Jsapi,
    Native,
    App
}

impl ToString for TradeType {
    fn to_string(&self) -> String {
        (match *self {
            TradeType::Jsapi => "JSAPI",
            TradeType::Native => "NATIVE",
            TradeType::App => "APP"
        }).to_string()
    }
}

/// 银行类型
pub enum BankType {}

/// [交易金额]:
/// 交易金额默认为人民币交易，接口中参数支付金额单位为【分】，参数值不能带小数。
/// 对账单中的交易金额单位为【元】。
/// 外币交易的支付金额精确到币种的最小单位，参数值不能带小数点。
pub fn get_trade_amount(v: f32) -> usize {
    // FIXME:: 不同情况下的金额处理
    (v * 100.0).round() as usize
}

/// [时间]:
/// 标准北京时间，时区为东八区；如果商户的系统时间为非标准北京时间。
/// 参数值必须根据商户系统所在时区先换算成标准北京时间，
/// 例如商户所在地为0时区的伦敦，当地时间为2014年11月11日0时0分0秒，
/// 换算成北京时间为2014年11月11日8时0分0秒。
pub fn get_time_str() -> String {
    // FIXME:: 如果是服务器在海外中国网站就会有问题
    strftime("%Y%m%d%H%M%S", &time::now()).unwrap()
}

/// [时间戳]:
/// 标准北京时间，时区为东八区，自1970年1月1日 0点0分0秒以来的秒数。
/// 注意：部分系统取到的值为毫秒级，需要转换成秒(10位数字)。
pub fn get_timestamp() -> i64 {
    time::get_time().sec
}

/// [生成随机数算法]:
/// 微信支付API接口协议中包含字段nonce_str，主要保证签名不可预测。
/// 我们推荐生成随机数算法如下：调用随机数函数生成，将得到的值转换为字符串。
pub fn get_nonce_str() -> String {
    Uuid::new_v4().simple().to_string()
}

/// [商户订单号]:
/// 商户支付的订单号由商户自定义生成，微信支付要求商户订单号保持唯一性
/// （建议根据当前系统时间加随机序列来生成订单号）。
/// 重新发起一笔支付要使用原订单号，避免重复支付；
/// 已支付过或已调用关单、撤销（请见后文的API列表）的订单号不能重新发起支付。
pub fn get_order_no() -> String {
    get_time_str() + &((&get_nonce_str())[..18])
}

pub fn sign(pairs: HashMap<String, String>) -> String {
    "".to_string()
}


#[cfg(test)]
mod tests {
    extern crate time;
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(get_time_str().len(), 14);
        assert_eq!(get_nonce_str().len(), 32);
        assert_eq!(get_order_no().len(), 32);
    }
}
