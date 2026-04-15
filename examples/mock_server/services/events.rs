use crate::helpers::soap;

pub fn resp_event_properties() -> String {
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl" xmlns:wstop="http://docs.oasis-open.org/wsn/t-1""#,
        r#"<tev:GetEventPropertiesResponse>
          <tev:TopicNamespaceLocation>http://www.onvif.org/onvif/ver10/topics/topicns.xml</tev:TopicNamespaceLocation>
          <wstop:FixedTopicSet>true</wstop:FixedTopicSet>
          <wstop:TopicSet>
            <tns1:VideoSource wstop:topic="false" xmlns:tns1="http://www.onvif.org/ver10/topics">
              <tns1:MotionAlarm wstop:topic="true"/>
            </tns1:VideoSource>
            <tns1:RuleEngine wstop:topic="false" xmlns:tns1="http://www.onvif.org/ver10/topics">
              <tns1:FieldDetector wstop:topic="false">
                <tns1:ObjectsInside wstop:topic="true"/>
              </tns1:FieldDetector>
            </tns1:RuleEngine>
          </wstop:TopicSet>
          <tev:TopicExpressionDialect>http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet</tev:TopicExpressionDialect>
        </tev:GetEventPropertiesResponse>"#,
    )
}

pub fn resp_create_pull_point_subscription() -> String {
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl" xmlns:wsa="http://www.w3.org/2005/08/addressing""#,
        r#"<tev:CreatePullPointSubscriptionResponse>
          <tev:SubscriptionReference>
            <wsa:Address>http://mock-server/onvif/events/subscription_1</wsa:Address>
          </tev:SubscriptionReference>
          <tev:CurrentTime>2026-04-05T00:00:00Z</tev:CurrentTime>
          <tev:TerminationTime>2026-04-05T00:01:00Z</tev:TerminationTime>
        </tev:CreatePullPointSubscriptionResponse>"#,
    )
}

pub fn resp_pull_messages() -> String {
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl" xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2" xmlns:tns1="http://www.onvif.org/ver10/topics""#,
        r#"<tev:PullMessagesResponse>
          <tev:CurrentTime>2026-04-05T00:00:01Z</tev:CurrentTime>
          <tev:TerminationTime>2026-04-05T00:01:00Z</tev:TerminationTime>
          <wsnt:NotificationMessage>
            <wsnt:Topic Dialect="http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet">tns1:VideoSource/MotionAlarm</wsnt:Topic>
            <wsnt:Message>
              <tt:Message UtcTime="2026-04-05T00:00:01Z" PropertyOperation="Changed">
                <tt:Source>
                  <tt:SimpleItem Name="VideoSourceToken" Value="VideoSource_1"/>
                </tt:Source>
                <tt:Data>
                  <tt:SimpleItem Name="IsMotion" Value="true"/>
                </tt:Data>
              </tt:Message>
            </wsnt:Message>
          </wsnt:NotificationMessage>
        </tev:PullMessagesResponse>"#,
    )
}

pub fn resp_subscribe() -> String {
    soap(
        r#"xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2" xmlns:wsa="http://www.w3.org/2005/08/addressing""#,
        r#"<wsnt:SubscribeResponse>
          <wsnt:SubscriptionReference>
            <wsa:Address>http://mock-server/onvif/events/push_sub_1</wsa:Address>
          </wsnt:SubscriptionReference>
          <wsnt:CurrentTime>2026-04-05T00:00:00Z</wsnt:CurrentTime>
          <wsnt:TerminationTime>2026-04-05T00:01:00Z</wsnt:TerminationTime>
        </wsnt:SubscribeResponse>"#,
    )
}

pub fn resp_renew() -> String {
    soap(
        r#"xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2""#,
        r#"<wsnt:RenewResponse>
          <wsnt:TerminationTime>2026-04-05T00:02:00Z</wsnt:TerminationTime>
          <wsnt:CurrentTime>2026-04-05T00:00:30Z</wsnt:CurrentTime>
        </wsnt:RenewResponse>"#,
    )
}
