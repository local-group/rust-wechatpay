
extern crate time;
extern crate uuid;
extern crate url;
extern crate md5;
extern crate xml;


use std::string::ToString;
use std::collections::HashMap;

use url::form_urlencoded;
use xml::writer::{events};
use time::{strftime};
use uuid::Uuid;


/// 货币种类: 人民币
pub const CURRENCY_CNY: &'static str = "CNY";
/// 统一下单 URL
pub const UNIFIEDORDER_URL: &'static str = "https://api.mch.weixin.qq.com/pay/unifiedorder";
/// 查询订单 URL
pub const ORDERQUERY_URL: &'static str = "https://api.mch.weixin.qq.com/pay/orderquery";


/// [交易类型]
pub enum TradeType {
    /// `JSAPI`
    Jsapi,
    /// `NATIVE`
    Native,
    /// `APP` : app支付，统一下单接口trade_type的传参可参考这里
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

/// [交易金额]
///
/// 交易金额默认为人民币交易，接口中参数支付金额单位为【分】，参数值不能带小数。
/// 对账单中的交易金额单位为【元】。
/// 外币交易的支付金额精确到币种的最小单位，参数值不能带小数点。
pub fn get_trade_amount(v: f32) -> usize {
    // FIXME:: 不同情况下的金额处理
    (v * 100.0).round() as usize
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
pub fn sign(pairs: &HashMap<String, String>) -> String {
    // 如果参数的值为空不参与签名；
    let mut keys = pairs
        .iter()
        .filter(|pair| {
            pair.0.ne("key") && pair.0.ne("sign") && pair.1.len() > (0 as usize)
        })
        .map(|pair| {pair.0.to_string()})
        .collect::<Vec<String>>();

    // 参数名ASCII码从小到大排序（字典序）；
    keys.sort();
    let mut encoder = form_urlencoded::Serializer::new(String::new());
    for key in keys {
        encoder.append_pair(&key, &pairs[&key]);
    }
    encoder.append_pair("key", pairs.get("key").unwrap());
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

/// 将`xml`数据解析成`HashMap`
pub fn from_xml_str(data: &str) -> HashMap<String, String> {
    let mut pairs = HashMap::new();

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

/// 使用`HashMap`生成`xml`数据
pub fn to_xml_str(pairs: &HashMap<String, String>) -> String {
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

    use std::collections::HashMap;

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

    fn check_xml_str(pairs: &HashMap<String, String>, data: &str) {
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
        let mut pairs = HashMap::new();
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
    fn test_string_length() {
        assert_eq!(format!("{}", ::get_timestamp()).len(), 10);
        assert_eq!(::get_time_str().len(), 14);
        assert_eq!(::get_nonce_str().len(), 32);
        assert_eq!(::get_order_no().len(), 32);
    }

    #[test]
    fn test_sign() {
        let mut pairs = HashMap::new();
        for &(k, v) in [
            ("appid"       , "wxd930ea5d5a258f4f"),
            ("mch_id"      , "10000100"),
            ("device_info" , "1000"),
            ("body"        , "test"),
            ("nonce_str"   , "ibuaiVcKdpRxkhJA"),
            ("key"         , "192006250b4c09247ec02edce69f6a2d"),
        ].iter() {
            pairs.insert(k.to_string(), v.to_string());
        }
        assert_eq!(::sign(&pairs), "9A0A8659F005D6984697E2CA0A9CF3B7");
    }
}
