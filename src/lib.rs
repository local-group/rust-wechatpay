
extern crate hyper;
extern crate curl;
extern crate time;
extern crate uuid;
extern crate url;
extern crate md5;
extern crate xml;


use std::io::{Read, Write};
use std::string::ToString;
use std::collections::BTreeMap;

use curl::easy::Easy;
use url::form_urlencoded;
use xml::writer::{events};
use time::{strftime};
use uuid::Uuid;


/// 货币种类: 人民币
const _CURRENCY_CNY: &'static str = "CNY";
/// 统一下单 URL
const UNIFIEDORDER_URL: &'static str = "https://api.mch.weixin.qq.com/pay/unifiedorder";
const MICROPAY_URL: &'static str = "https://api.mch.weixin.qq.com/pay/micropay";
/// 查询订单 URL
const ORDERQUERY_URL: &'static str = "https://api.mch.weixin.qq.com/pay/orderquery";


impl ToString for TradeType {
    fn to_string(&self) -> String {
        (match *self {
            TradeType::Micro => "MICRO",
            TradeType::Jsapi => "JSAPI",
            TradeType::Native | TradeType::Qrcode => "NATIVE",
            TradeType::App => "APP"
        }).to_string()
    }
}

/// 银行类型
pub enum BankType {}

enum ParamsCheckType {
    Required,
    Forbidden
}

/// 错误类
pub enum WechatpayError {
    /// 缺少字段
    MissingField(String),
    /// 多余的字段
    RedundantField(String),
    Curl(curl::Error),
    Request,
    Unknown
}

/// 订单标识
pub enum OrderIdentifier {
    TransactionId(String),
    OutTradeNo(String)
}

/// API 请求结果
pub type WechatpayResult = Result<BTreeMap<String, String>, WechatpayError>;


/// API Client
pub struct WechatpayClient {
    appid: String,
    mch_id: String,
    api_key: String,
    notify_url: String,
    cert: String, // unused
}

impl WechatpayClient {
    pub fn new(appid: &str, mch_id: &str, api_key: &str, notify_url: &str, cert: &str) -> WechatpayClient {
        WechatpayClient{
            appid: appid.to_string(),
            mch_id: mch_id.to_string(),
            api_key: api_key.to_string(),
            notify_url: notify_url.to_string(),
            cert: cert.to_string()
        }
    }

    fn check_params(&self,
                    params: &BTreeMap<String, String>,
                    keys: Vec<&str>,
                    check_type: ParamsCheckType) -> Option<WechatpayError> {
        for key in keys.iter() {
            match check_type {
                ParamsCheckType::Required => {
                    if params.get(&key.to_string()).unwrap_or(&"".to_string()).is_empty() {
                        return Some(WechatpayError::MissingField(key.to_string()));
                    }
                }
                ParamsCheckType::Forbidden => {
                    if params.get(&key.to_string()).unwrap_or(&"".to_string()).is_empty() {
                        return Some(WechatpayError::RedundantField(key.to_string()));
                    }
                }
            }
        }
        None
    }

    fn request(&self,
               url: &str,
               params: BTreeMap<String, String>,
               retries: Option<u32>,
               require_cert: bool) -> WechatpayResult {

        let api_key = self.api_key.to_string();
        let sign_str = get_sign(&params, &api_key);
        let mut params = params;
        params.insert("sign".to_string(), sign_str);

        let xml_str = to_xml_str(&params);
        let mut handle = Easy::new();
        let mut err = WechatpayError::Request;
        let _ = handle.url(url).map_err(|e| {
            err = WechatpayError::Curl(e);
        });
        if require_cert {
            let _ = handle.ssl_cert(&self.cert).map_err(|e| {
                err = WechatpayError::Curl(e);
            });
        }
        let _ = handle.read_function(move |buf| {
            Ok(xml_str.as_bytes().read(buf).unwrap_or(0))
        }).map_err(|e| {
            err = WechatpayError::Curl(e);
        });

        for _ in 0..retries.unwrap_or(1) {
            let mut data = Vec::<u8>::new();
            {
                let mut handle = handle.transfer();
                let _ = handle.write_function(|text| {
                    Ok(match data.write_all(text) {
                        Ok(_) => text.len(),
                        Err(_) => 0
                    })
                }).map_err(|e| {
                    err = WechatpayError::Curl(e);
                });
                let _ = handle.perform().map_err(|e|{
                    err = WechatpayError::Curl(e);
                });
            }

            let status_code = match handle.response_code() {
                Ok(code) => code,
                Err(e) => {
                    err = WechatpayError::Curl(e);
                    0
                }
            };
            if status_code == 200 || status_code == 201 {
                let s = String::from_utf8(data).unwrap();
                return Ok(from_xml_str(s.as_ref()))
            }
        }
        Err(err)
    }

    // let retries = if retries == 0 { 3 } else { retries };
    pub fn pay(&self,
               params: BTreeMap<String, String>,
               trade_type: TradeType,
               retries: Option<u32>) -> WechatpayResult {
        if let Some(e) = self.check_params(&params, vec!["key", "sign"],
                                           ParamsCheckType::Forbidden) {
            return Err(e);
        }
        if let Some(e) = self.check_params(&params,
                                           vec!["body", "out_trade_no", "total_fee", "spbill_create_ip"],
                                           ParamsCheckType::Required) {
            return Err(e);
        }
        match trade_type {
            TradeType::Native => {
                if let Some(e) = self.check_params(&params, vec!["product_id"], ParamsCheckType::Required) {
                    return Err(e);
                }
            }
            TradeType::Jsapi => {
                if let Some(e) = self.check_params(&params, vec!["openid"], ParamsCheckType::Required) {
                    return Err(e);
                }
            }
            TradeType::Micro => {
                if let Some(e) = self.check_params(&params, vec!["auth_code"], ParamsCheckType::Required) {
                    return Err(e);
                }
            }
            _ => {}
        }

        let url = if trade_type == TradeType::Micro { MICROPAY_URL } else { UNIFIEDORDER_URL };
        let body = params.get("body").unwrap_or(&"Test Request".to_string()).to_string();
        let mut params = params;
        params.insert("trade_type".to_string(), trade_type.to_string());
        params.insert("appid".to_string(), self.appid.clone());
        params.insert("mch_id".to_string(), self.mch_id.clone());
        params.insert("nonce_str".to_string(), get_nonce_str());
        params.insert("body".to_string(), body);
        if trade_type != TradeType::Micro {
            params.insert("notify_url".to_string(), self.notify_url.clone());
        }
        self.request(url, params, retries, false)
    }

    pub fn micro_pay(&self,
                   params: BTreeMap<String, String>,
                   retries: Option<u32>) -> WechatpayResult {
        self.pay(params, TradeType::Micro, retries)
    }

    pub fn jsapi_pay(&self,
                   params: BTreeMap<String, String>,
                   retries: Option<u32>) -> WechatpayResult {
        self.pay(params, TradeType::Jsapi, retries)
    }

    pub fn qrcode_pay(&self,
                   params: BTreeMap<String, String>,
                   retries: Option<u32>) -> WechatpayResult {
        self.pay(params, TradeType::Qrcode, retries)
    }

    pub fn app_pay(&self,
                   params: BTreeMap<String, String>,
                   retries: Option<u32>) -> WechatpayResult {
        self.pay(params, TradeType::App, retries)
    }

    pub fn query_order(&self, id: OrderIdentifier) -> WechatpayResult {
        let mut params = BTreeMap::new();
        match id {
            OrderIdentifier::TransactionId(s) => {
                params.insert("transaction_id".to_string(), s);
            }
            OrderIdentifier::OutTradeNo(s) => {
                params.insert("out_trade_no".to_string(), s);
            }
        }
        params.insert("appid".to_string(), self.appid.clone());
        params.insert("mch_id".to_string(), self.mch_id.clone());
        params.insert("nonce_str".to_string(), get_nonce_str());

        self.request(ORDERQUERY_URL, params, None, false)
    }
}

/// [交易类型]
#[derive(PartialEq)]
pub enum TradeType {
    /// `MICRO`
    Micro,
    /// `JSAPI`
    Jsapi,
    /// `NATIVE`
    Native, Qrcode,
    /// `APP` : app支付，统一下单接口trade_type的传参可参考这里
    App
}

/// [交易金额]
///
/// 交易金额默认为人民币交易，接口中参数支付金额单位为【分】，参数值不能带小数。
/// 对账单中的交易金额单位为【元】。
/// 外币交易的支付金额精确到币种的最小单位，参数值不能带小数点。
pub fn get_trade_amount(v: f32) -> u32 {
    // FIXME:: 不同情况下的金额处理
    (v * 100.0).round() as u32
}

/// [时间]
///
/// 标准北京时间，时区为东八区；如果商户的系统时间为非标准北京时间。
/// 参数值必须根据商户系统所在时区先换算成标准北京时间，
/// 例如商户所在地为0时区的伦敦，当地时间为2014年11月11日0时0分0秒，
/// 换算成北京时间为2014年11月11日8时0分0秒。
pub fn get_time_str() -> String {
    // FIXME:: 如果是服务器在海外中国网站就会有问题
    strftime("%Y%m%d%H%M%S", &time::now()).unwrap()
}

/// [时间戳]
///
/// 标准北京时间，时区为东八区，自1970年1月1日 0点0分0秒以来的秒数。
/// 注意：部分系统取到的值为毫秒级，需要转换成秒(10位数字)。
pub fn get_timestamp() -> i64 {
    time::get_time().sec
}

/// [生成随机数算法]
///
/// 微信支付API接口协议中包含字段nonce_str，主要保证签名不可预测。
/// 我们推荐生成随机数算法如下：调用随机数函数生成，将得到的值转换为字符串。
pub fn get_nonce_str() -> String {
    Uuid::new_v4().simple().to_string()
}

/// [商户订单号]
///
/// 商户支付的订单号由商户自定义生成，微信支付要求商户订单号保持唯一性
/// （建议根据当前系统时间加随机序列来生成订单号）。
/// 重新发起一笔支付要使用原订单号，避免重复支付；
/// 已支付过或已调用关单、撤销（请见后文的API列表）的订单号不能重新发起支付。
pub fn get_order_no() -> String {
    get_time_str() + &((&get_nonce_str())[..18])
}

/// 签名算法 (给请求参数签名)
///
/// 详见: 接口规则 > 安全规范
pub fn get_sign(pairs: &BTreeMap<String, String>, api_key: &String) -> String {
    // 如果参数的值为空不参与签名；
    let keys = pairs
        .iter()
        .filter(|pair| {
            pair.0.ne("key") && pair.0.ne("sign") && !pair.1.is_empty()
        })
        .map(|pair| {pair.0.to_string()})
        .collect::<Vec<String>>();

    // 参数名ASCII码从小到大排序（字典序）；
    let mut encoder = form_urlencoded::Serializer::new(String::new());
    for key in keys {
        encoder.append_pair(&key, &pairs[&key]);
    }

    encoder.append_pair("key", api_key);
    let encoded = encoder.finish();

    // 生成 MD5 字符串
    let mut context = md5::Context::new();
    context.consume(encoded.as_bytes());
    let mut digest = String::with_capacity(32);
    for x in &context.compute()[..] {
        digest.push_str(&format!("{:02X}", x));
    }
    digest
}

/// 将`xml`数据解析成`BTreeMap`
pub fn from_xml_str(data: &str) -> BTreeMap<String, String> {
    let mut pairs = BTreeMap::new();

    let reader = xml::reader::EventReader::from_str(data);
    let mut tag: String = "".to_string();
    for event in reader {
        match event {
            Ok(xml::reader::XmlEvent::StartElement{name, ..}) => {
                tag = name.local_name;
            }
            Ok(xml::reader::XmlEvent::CData(value)) => {
                pairs.insert(tag.clone(), value);
            }
            Err(e) => {
                println!("Parse xml error: {:?}", e);
                break;
            }
            _ => {}
        }
    }
    pairs
}

/// 使用`BTreeMap`生成`xml`数据
pub fn to_xml_str(pairs: &BTreeMap<String, String>) -> String {
    let mut target: Vec<u8> = Vec::new();
    {
        let mut writer = xml::writer::EmitterConfig::new()
            .write_document_declaration(false)
            .create_writer(&mut target);
        let _ = writer.write::<events::XmlEvent>(events::XmlEvent::start_element("xml").into());
        for (key, value) in pairs{
            let _ = writer.write::<events::XmlEvent>(events::XmlEvent::start_element(key.as_ref()).into());
            let _ = writer.write::<events::XmlEvent>(events::XmlEvent::characters(value.as_ref()).into());
            let _ = writer.write::<events::XmlEvent>(events::XmlEvent::end_element().into());
        }
        let _ = writer.write::<events::XmlEvent>(events::XmlEvent::end_element().into());
    }
    String::from_utf8(target).unwrap()
}


#[cfg(test)]
mod tests {
    extern crate time;
    extern crate xml;

    use std::collections::BTreeMap;

    use xml::reader::{EventReader, XmlEvent};

    #[test]
    fn test_from_xml_str() {
        let source = r#"
<xml>
   <return_code><![CDATA[SUCCESS]]></return_code>
   <return_msg><![CDATA[OK]]></return_msg>
   <appid><![CDATA[wx2421b1c4370ec43b]]></appid>
   <mch_id><![CDATA[10000100]]></mch_id>
   <device_info><![CDATA[1000]]></device_info>
   <nonce_str><![CDATA[TN55wO9Pba5yENl8]]></nonce_str>
   <sign><![CDATA[BDF0099C15FF7BC6B1585FBB110AB635]]></sign>
   <result_code><![CDATA[SUCCESS]]></result_code>
   <openid><![CDATA[oUpF8uN95-Ptaags6E_roPHg7AG0]]></openid>
   <is_subscribe><![CDATA[Y]]></is_subscribe>
   <trade_type><![CDATA[APP]]></trade_type>
   <bank_type><![CDATA[CCB_DEBIT]]></bank_type>
   <total_fee>1</total_fee>
   <fee_type><![CDATA[CNY]]></fee_type>
   <transaction_id><![CDATA[1008450740201411110005820873]]></transaction_id>
   <out_trade_no><![CDATA[1415757673]]></out_trade_no>
   <attach><![CDATA[订单额外描述]]></attach>
   <time_end><![CDATA[20141111170043]]></time_end>
   <trade_state><![CDATA[SUCCESS]]></trade_state>
</xml>
"#;
        let pairs = ::from_xml_str(source);
        for &(k, v) in [
            ("return_code"    , "SUCCESS"),
            ("return_msg"     , "OK"),
            ("appid"          , "wx2421b1c4370ec43b"),
            ("mch_id"         , "10000100"),
            ("result_code"    , "SUCCESS"),
            ("attach"         , "订单额外描述"),
            ("transaction_id" , "1008450740201411110005820873"),
            ("time_end"       , "20141111170043"),
            ("trade_type"     , "APP")
        ].iter() {
            assert_eq!(pairs.get(k), Some(&v.to_string()));
        }
    }

    fn check_xml_str(pairs: &BTreeMap<String, String>, data: &str) {
        let reader = EventReader::from_str(data);
        let mut tag: String = "".to_string();
        for event in reader {
            match event {
                Ok(XmlEvent::StartElement{name, ..}) => {
                    tag = name.local_name;
                }
                Ok(XmlEvent::Characters(s)) => {
                    assert_eq!(Some(&s), pairs.get(&tag));
                }
                Err(e) => {
                    panic!(format!("Parse error: {:?}", e));
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_to_xml_str() {
        let output = r#"
<xml>
   <appid>wx2421b1c4370ec43b</appid>
   <attach>支付测试</attach>
   <body>APP支付测试</body>
   <mch_id>10000100</mch_id>
   <nonce_str>1add1a30ac87aa2db72f57a2375d8fec</nonce_str>
   <notify_url>http://wxpay.weixin.qq.com/pub_v2/pay/notify.v2.php</notify_url>
   <out_trade_no>1415659990</out_trade_no>
   <spbill_create_ip>14.23.150.211</spbill_create_ip>
   <total_fee>1</total_fee>
   <trade_type>APP</trade_type>
   <sign>0CB01533B8C1EF103065174F50BCA001</sign>
</xml>
"#;
        let mut pairs = BTreeMap::new();
        for &(k, v) in [
            ("appid"            , "wx2421b1c4370ec43b"),
            ("attach"           , "支付测试"),
            ("body"             , "APP支付测试"),
            ("mch_id"           , "10000100"),
            ("nonce_str"        , "1add1a30ac87aa2db72f57a2375d8fec"),
            ("notify_url"       , "http://wxpay.weixin.qq.com/pub_v2/pay/notify.v2.php"),
            ("out_trade_no"     , "1415659990"),
            ("spbill_create_ip" , "14.23.150.211"),
            ("total_fee"        , "1"),
            ("trade_type"       , "APP"),
            ("sign"             , "0CB01533B8C1EF103065174F50BCA001")
        ].iter() {
            pairs.insert(k.to_string(), v.to_string());
        }

        check_xml_str(&pairs, output);
        check_xml_str(&pairs, &(::to_xml_str(&pairs)));
    }

    #[test]
    fn test_trade_amount() {
        assert_eq!(::get_trade_amount(0.99), 99_u32);
        assert_eq!(::get_trade_amount(0.999), 100_u32);
        assert_eq!(::get_trade_amount(3.3), 330_u32);
        assert_eq!(::get_trade_amount(20_f32), 2000_u32);
    }

    #[test]
    fn test_string_length() {
        assert_eq!(format!("{}", ::get_timestamp()).len(), 10);
        assert_eq!(::get_time_str().len(), 14);
        assert_eq!(::get_nonce_str().len(), 32);
        assert_eq!(::get_order_no().len(), 32);
    }

    #[test]
    fn test_sign() {
        let mut pairs = BTreeMap::new();
        for &(k, v) in [
            ("appid"       , "wxd930ea5d5a258f4f"),
            ("mch_id"      , "10000100"),
            ("device_info" , "1000"),
            ("body"        , "test"),
            ("nonce_str"   , "ibuaiVcKdpRxkhJA")
        ].iter() {
            pairs.insert(k.to_string(), v.to_string());
        }
        let api_key = "192006250b4c09247ec02edce69f6a2d".to_string();
        assert_eq!(::get_sign(&pairs, &api_key), "9A0A8659F005D6984697E2CA0A9CF3B7");
    }
}
